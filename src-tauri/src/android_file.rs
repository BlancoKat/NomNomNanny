//! Read and write Android Storage Access Framework URIs.
//!
//! `tauri-plugin-fs` opens these URIs with `ParcelFileDescriptor.detachFd()`.
//! The content provider then records a 0-byte file before the bytes are written.
//! Keeping the Java `OutputStream` open until `close()` avoids that.

use jni::objects::{JByteArray, JObject, JValue};
use jni::JNIEnv;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, Result};

pub fn read_uri(app: &AppHandle, uri: &str) -> Result<Vec<u8>> {
    let uri = uri.to_string();
    with_activity(app, move |env, activity| read_stream(env, activity, &uri))
}

pub fn write_uri(app: &AppHandle, uri: &str, contents: &str) -> Result<()> {
    let contents = contents.to_string();
    let uri = uri.to_string();
    with_activity(app, move |env, activity| {
        write_stream(env, activity, &uri, &contents)
    })
}

fn with_activity<T: Send + 'static>(
    app: &AppHandle,
    task: impl FnOnce(&mut JNIEnv, &JObject) -> std::result::Result<T, String> + Send + 'static,
) -> Result<T> {
    let webview = app.get_webview_window("main").ok_or_else(|| {
        AppError::Internal("The Android window is not open, so the file could not be saved.".into())
    })?;

    // `with_webview` only hands back the handle. The JNI work runs afterwards
    // on the Android UI thread, and this command thread waits for it.
    let (handle_tx, handle_rx) = std::sync::mpsc::channel();
    webview
        .with_webview(move |platform| {
            let _ = handle_tx.send(platform.jni_handle());
        })
        .map_err(|err| AppError::Internal(err.to_string()))?;
    let handle = handle_rx.recv().map_err(|_| {
        AppError::Internal("The Android window closed before the file could be opened.".into())
    })?;

    let (result_tx, result_rx) = std::sync::mpsc::channel();
    handle.exec(move |env, activity, _webview| {
        let _ = result_tx.send(task(env, activity));
    });
    result_rx
        .recv()
        .map_err(|_| AppError::Internal("Android did not finish opening the file.".into()))?
        .map_err(AppError::Internal)
}

fn read_stream(
    env: &mut JNIEnv,
    activity: &JObject,
    uri: &str,
) -> std::result::Result<Vec<u8>, String> {
    let parsed = parse_uri(env, uri)?;
    let resolver = content_resolver(env, activity)?;
    let stream = env
        .call_method(
            &resolver,
            "openInputStream",
            "(Landroid/net/Uri;)Ljava/io/InputStream;",
            &[JValue::Object(&parsed)],
        )
        .map_err(|err| explain(env, err))?
        .l()
        .map_err(|err| err.to_string())?;
    if stream.as_raw().is_null() {
        return Err("Android could not open that diary file.".into());
    }

    let buffer = env.new_byte_array(8192).map_err(|err| explain(env, err))?;
    let output = env
        .new_object("java/io/ByteArrayOutputStream", "()V", &[])
        .map_err(|err| explain(env, err))?;

    loop {
        let read = env
            .with_local_frame(8, |env| {
                env.call_method(&stream, "read", "([B)I", &[JValue::Object(&*buffer)])?
                    .i()
            })
            .map_err(|err: jni::errors::Error| explain(env, err))?;
        if read < 0 {
            break;
        }
        if read == 0 {
            continue;
        }
        env.with_local_frame(8, |env| {
            env.call_method(
                &output,
                "write",
                "([BII)V",
                &[JValue::Object(&*buffer), JValue::Int(0), JValue::Int(read)],
            )?;
            Ok::<(), jni::errors::Error>(())
        })
        .map_err(|err: jni::errors::Error| explain(env, err))?;
    }

    close_stream(env, &stream)?;
    let bytes = env
        .call_method(&output, "toByteArray", "()[B", &[])
        .map_err(|err| explain(env, err))?
        .l()
        .map_err(|err| err.to_string())?;
    let array = JByteArray::from(bytes);
    env.convert_byte_array(&array)
        .map_err(|err| explain(env, err))
}

fn write_stream(
    env: &mut JNIEnv,
    activity: &JObject,
    uri: &str,
    contents: &str,
) -> std::result::Result<(), String> {
    let parsed = parse_uri(env, uri)?;
    let resolver = content_resolver(env, activity)?;
    let mode = env.new_string("wt").map_err(|err| explain(env, err))?;
    let stream = env
        .call_method(
            &resolver,
            "openOutputStream",
            "(Landroid/net/Uri;Ljava/lang/String;)Ljava/io/OutputStream;",
            &[JValue::Object(&parsed), JValue::Object(&*mode)],
        )
        .map_err(|err| explain(env, err))?
        .l()
        .map_err(|err| err.to_string())?;
    if stream.as_raw().is_null() {
        return Err("Android could not open that diary file for saving.".into());
    }

    let bytes = env
        .byte_array_from_slice(contents.as_bytes())
        .map_err(|err| explain(env, err))?;
    let write_result = env
        .call_method(&stream, "write", "([B)V", &[JValue::Object(&*bytes)])
        .map(|_| ())
        .map_err(|err| explain(env, err));
    let flush_result = if write_result.is_ok() {
        env.call_method(&stream, "flush", "()V", &[])
            .map(|_| ())
            .map_err(|err| explain(env, err))
    } else {
        Ok(())
    };
    let close_result = close_stream(env, &stream);
    write_result?;
    flush_result?;
    close_result
}

fn parse_uri<'a>(env: &mut JNIEnv<'a>, uri: &str) -> std::result::Result<JObject<'a>, String> {
    let uri_string = env.new_string(uri).map_err(|err| explain(env, err))?;
    env.call_static_method(
        "android/net/Uri",
        "parse",
        "(Ljava/lang/String;)Landroid/net/Uri;",
        &[JValue::Object(&*uri_string)],
    )
    .map_err(|err| explain(env, err))?
    .l()
    .map_err(|err| err.to_string())
}

fn content_resolver<'a>(
    env: &mut JNIEnv<'a>,
    activity: &JObject,
) -> std::result::Result<JObject<'a>, String> {
    env.call_method(
        activity,
        "getContentResolver",
        "()Landroid/content/ContentResolver;",
        &[],
    )
    .map_err(|err| explain(env, err))?
    .l()
    .map_err(|err| err.to_string())
}

fn close_stream(env: &mut JNIEnv, stream: &JObject) -> std::result::Result<(), String> {
    env.call_method(stream, "close", "()V", &[])
        .map(|_| ())
        .map_err(|err| explain(env, err))
}

fn explain(env: &JNIEnv, err: jni::errors::Error) -> String {
    let _ = env.exception_describe();
    let _ = env.exception_clear();
    err.to_string()
}
