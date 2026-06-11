use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::db::DbState;
use crate::error::{AppError, Result};

/// Lightweight search result for the UI list
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsdaSearchHit {
    pub fdc_id: i64,
    pub description: String,
    pub data_type: Option<String>,
    pub brand_owner: Option<String>,
    pub food_category: Option<String>,
}

/// Clean, UI-friendly representation of a food with portions
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CachedFood {
    pub fdc_id: i64,
    pub description: String,
    pub data_type: Option<String>,
    pub brand_owner: Option<String>,
    pub nutrients_per_100g: MacroNutrients,
    pub portions: Vec<FoodPortion>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MacroNutrients {
    pub kcal: f64,
    pub protein: f64,
    pub fat: f64,
    pub carbs: f64,
    pub fiber: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FoodPortion {
    pub label: String,
    pub grams: f64,
}

// =============== USDA RAW RESPONSE TYPES (partial) ===============

#[derive(Debug, Deserialize)]
struct SearchResponse {
    foods: Option<Vec<SearchFood>>,
}

#[derive(Debug, Deserialize)]
struct SearchFood {
    #[serde(rename = "fdcId")]
    fdc_id: i64,
    description: String,
    #[serde(rename = "dataType")]
    data_type: Option<String>,
    #[serde(rename = "brandOwner")]
    brand_owner: Option<String>,
    #[serde(rename = "foodCategory")]
    food_category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FoodDetailResponse {
    #[serde(rename = "fdcId")]
    fdc_id: i64,
    description: String,
    #[serde(rename = "dataType")]
    data_type: Option<String>,
    #[serde(rename = "brandOwner")]
    brand_owner: Option<String>,
    #[serde(rename = "foodNutrients")]
    food_nutrients: Option<Vec<FoodNutrient>>,
    #[serde(rename = "foodPortions")]
    food_portions: Option<Vec<FoodPortionRaw>>,
}

#[derive(Debug, Deserialize)]
struct FoodNutrient {
    amount: Option<f64>,
    #[serde(rename = "nutrientId")]
    nutrient_id: Option<i64>,
    nutrient: Option<NutrientRef>,
}

#[derive(Debug, Deserialize)]
struct NutrientRef {
    id: Option<i64>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FoodPortionRaw {
    #[serde(rename = "gramWeight")]
    gram_weight: Option<f64>,
    amount: Option<f64>,
    #[serde(rename = "portionDescription")]
    portion_description: Option<String>,
    modifier: Option<String>,
    #[serde(rename = "measureUnit")]
    measure_unit: Option<MeasureUnit>,
}

#[derive(Debug, Deserialize)]
struct MeasureUnit {
    name: Option<String>,
    abbreviation: Option<String>,
}

// =============== HTTP CLIENT (simple per-call is fine for desktop) ===============

fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("NomNomNanny/0.1 (desktop nutrition app)")
        .build()
        .expect("failed to build HTTP client")
}

/// Search USDA FDC. Returns lightweight hits (no nutrients yet).
pub async fn search_usda_foods(query: String, api_key: String) -> Result<Vec<UsdaSearchHit>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    if api_key.trim().is_empty() {
        return Err(AppError::InvalidInput("USDA API key is required".into()));
    }

    let url = "https://api.nal.usda.gov/fdc/v1/foods/search";
    let params = [
        ("api_key", api_key.as_str()),
        ("query", query.trim()),
        ("dataType", "Foundation,SR Legacy"),
        ("pageSize", "25"),
        ("requireAllWords", "false"),
    ];

    let resp = http_client()
        .get(url)
        .query(&params)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("USDA search request failed: {e}")))?;

    if resp.status() == 429 {
        return Err(AppError::Internal("USDA rate limit reached. Please wait a minute or use Custom entry.".into()));
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!("USDA search error ({status}): {text}")));
    }

    // Read raw body for better error messages on deserialization failure
    let raw_body = resp.text().await.unwrap_or_default();

    let body: SearchResponse = serde_json::from_str(&raw_body)
        .map_err(|e| AppError::Internal(format!(
            "Failed to parse USDA search response: {}. Raw response: {}",
            e,
            if raw_body.len() > 500 { &raw_body[..500] } else { &raw_body }
        )))?;

    let hits = body
        .foods
        .unwrap_or_default()
        .into_iter()
        .map(|f| UsdaSearchHit {
            fdc_id: f.fdc_id,
            description: f.description,
            data_type: f.data_type,
            brand_owner: f.brand_owner,
            food_category: f.food_category,
        })
        .collect();

    Ok(hits)
}

/// Fetch full details for one food, normalize it, cache it, and return the clean `CachedFood`.
pub async fn fetch_and_cache_food_details(
    fdc_id: i64,
    api_key: String,
    db: DbState,
) -> Result<CachedFood> {
    if api_key.trim().is_empty() {
        return Err(AppError::InvalidInput("USDA API key is required".into()));
    }

    // 1. (Optional future) check full cached object here.
    // For v1 we always fetch so portions and data stay fresh.

    // 2. Fetch from USDA (full record — we extract the 5 nutrients ourselves)
    let url = format!("https://api.nal.usda.gov/fdc/v1/food/{fdc_id}");
    let params = [
        ("api_key", api_key.as_str()),
    ];

    let resp = http_client()
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("USDA food detail request failed: {e}")))?;

    if resp.status() == 429 {
        return Err(AppError::Internal("USDA rate limit reached. Please wait a minute.".into()));
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!("USDA detail error ({status}): {text}")));
    }

    let raw: FoodDetailResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse USDA food detail: {e}")))?;

    // 3. Normalize nutrients (per 100g)
    let nutrients = extract_macro_nutrients(&raw.food_nutrients.unwrap_or_default());

    // 4. Build nice portion list
    let mut portions = build_portions_list(&raw.food_portions.unwrap_or_default());

    // Always add reliable fallbacks
    if !portions.iter().any(|p| (p.grams - 100.0).abs() < 0.1) {
        portions.insert(0, FoodPortion {
            label: "100 g".to_string(),
            grams: 100.0,
        });
    }
    if !portions.iter().any(|p| (p.grams - 28.35).abs() < 0.5) {
        portions.push(FoodPortion {
            label: "1 oz (28.35 g)".to_string(),
            grams: 28.35,
        });
    }
    // Add "Custom grams" as a sentinel the UI can detect
    portions.push(FoodPortion {
        label: "Custom grams...".to_string(),
        grams: -1.0, // sentinel
    });

    let food = CachedFood {
        fdc_id: raw.fdc_id,
        description: raw.description,
        data_type: raw.data_type,
        brand_owner: raw.brand_owner,
        nutrients_per_100g: nutrients,
        portions,
    };

    // 5. Persist to cache (store full object as JSON for simplicity in v1)
    {
        let conn = db.lock().map_err(|e| AppError::Internal(e.to_string()))?;
        let nutrients_json = serde_json::to_string(&food.nutrients_per_100g)?;
        let portions_json = serde_json::to_string(&food.portions)?;

        conn.execute(
            "INSERT OR REPLACE INTO food_cache (fdc_id, description, data_type, brand_owner, nutrients_json, portions_json, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)",
            rusqlite::params![
                food.fdc_id,
                &food.description,
                &food.data_type,
                &food.brand_owner,
                nutrients_json,
                portions_json,
            ],
        )?;
    }

    Ok(food)
}

// =============== HELPERS ===============

fn extract_macro_nutrients(nutrients: &[FoodNutrient]) -> MacroNutrients {
    let mut out = MacroNutrients::default();

    for n in nutrients {
        let amount = match n.amount {
            Some(a) if a > 0.0 => a,
            _ => continue,
        };

        let id = n.nutrient_id
            .or_else(|| n.nutrient.as_ref().and_then(|nr| nr.id))
            .unwrap_or(0);

        match id {
            1008 => out.kcal = amount,
            1003 => out.protein = amount,
            1004 => out.fat = amount,
            1005 => out.carbs = amount,
            1079 => out.fiber = amount,
            _ => {}
        }
    }

    out
}

fn build_portions_list(raw_portions: &[FoodPortionRaw]) -> Vec<FoodPortion> {
    let mut out = Vec::new();

    for p in raw_portions {
        let grams = match p.gram_weight {
            Some(g) if g > 0.0 => g,
            _ => continue,
        };

        let label = if let Some(desc) = &p.portion_description {
            desc.clone()
        } else {
            let amt = p.amount.unwrap_or(1.0);
            let unit = p
                .measure_unit
                .as_ref()
                .and_then(|u| u.name.as_deref())
                .or(p.modifier.as_deref())
                .unwrap_or("serving");
            if amt == 1.0 {
                format!("1 {}", unit)
            } else {
                format!("{} {}", amt, unit)
            }
        };

        // Clean up ugly "undetermined" labels from some FDC records
        let label = if label.to_lowercase().contains("undetermined") {
            format!("{:.0} g", grams)
        } else {
            format!("{} ({:.0} g)", label, grams)
        };

        out.push(FoodPortion { label, grams });
    }

    // Deduplicate by grams (keep first)
    let mut seen = HashMap::new();
    out.retain(|p| seen.insert((p.grams * 100.0) as i64, true).is_none());

    out.sort_by(|a, b| a.grams.partial_cmp(&b.grams).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Pure function: given nutrients per 100g and desired grams, return the scaled macros.
pub fn scale_nutrients(nutrients: &MacroNutrients, grams: f64) -> MacroNutrients {
    if grams <= 0.0 {
        return MacroNutrients::default();
    }
    let factor = grams / 100.0;
    MacroNutrients {
        kcal: (nutrients.kcal * factor * 10.0).round() / 10.0,
        protein: (nutrients.protein * factor * 100.0).round() / 100.0,
        fat: (nutrients.fat * factor * 100.0).round() / 100.0,
        carbs: (nutrients.carbs * factor * 100.0).round() / 100.0,
        fiber: (nutrients.fiber * factor * 100.0).round() / 100.0,
    }
}

// =============== TAURI COMMAND WRAPPERS ===============

#[tauri::command]
pub async fn search_usda_foods_cmd(query: String, apiKey: String) -> std::result::Result<Vec<UsdaSearchHit>, AppError> {
    search_usda_foods(query, apiKey).await
}

#[tauri::command]
pub async fn fetch_usda_food_details_cmd(
    fdcId: i64,
    apiKey: String,
    state: tauri::State<'_, DbState>,
) -> std::result::Result<CachedFood, AppError> {
    fetch_and_cache_food_details(fdcId, apiKey, state.inner().clone()).await
}

/// Expose the exact same scaling math the backend uses (for live preview in UI)
#[tauri::command]
pub fn scale_nutrients_cmd(nutrients: MacroNutrients, grams: f64) -> MacroNutrients {
    scale_nutrients(&nutrients, grams)
}