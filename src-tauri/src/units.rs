//! Serving units for foods whose labels are not per gram.
//!
//! Nutrient rows stay per 100 g, which is what already-saved foods and intake
//! entries use. A volume amount becomes grams through a density. The default
//! density is 1 g/mL (water). A custom food can store a different density when
//! the label is per mL and the food is not water-like. Weight ounces are
//! 28.349523125 g. A US fluid ounce is 29.5735295625 mL, then the same density.

use serde::Deserialize;

use crate::error::{AppError, Result};

pub const GRAMS_PER_OUNCE: f64 = 28.349523125;
pub const ML_PER_FLUID_OUNCE: f64 = 29.5735295625;
pub const DEFAULT_DENSITY_G_PER_ML: f64 = 1.0;

#[derive(Debug, Deserialize)]
pub struct ServingAmount {
    pub amount: f64,
    pub unit: String,
    #[serde(default)]
    pub density_g_per_ml: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServingUnit {
    Gram,
    Milliliter,
    Ounce,
    FluidOunce,
}

impl ServingUnit {
    pub fn canonical(self) -> &'static str {
        match self {
            ServingUnit::Gram => "g",
            ServingUnit::Milliliter => "ml",
            ServingUnit::Ounce => "oz",
            ServingUnit::FluidOunce => "fl oz",
        }
    }

    pub fn is_volume(self) -> bool {
        matches!(self, ServingUnit::Milliliter | ServingUnit::FluidOunce)
    }
}

pub fn parse_serving_unit(unit: &str) -> Result<ServingUnit> {
    let normalized = unit
        .trim()
        .to_lowercase()
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match normalized.as_str() {
        "g" | "gram" | "grams" => Ok(ServingUnit::Gram),
        "ml" | "milliliter" | "milliliters" | "millilitre" | "millilitres" => {
            Ok(ServingUnit::Milliliter)
        }
        "oz" | "ounce" | "ounces" | "wt oz" | "weight oz" => Ok(ServingUnit::Ounce),
        "fl oz" | "floz" | "fluid oz" | "fluid ounce" | "fluid ounces" => {
            Ok(ServingUnit::FluidOunce)
        }
        _ => Err(AppError::InvalidInput(format!(
            "Unknown serving unit \"{unit}\". Use g, mL, oz, or fl oz."
        ))),
    }
}

pub fn density_or_default(density_g_per_ml: Option<f64>) -> Result<f64> {
    match density_g_per_ml {
        None => Ok(DEFAULT_DENSITY_G_PER_ML),
        Some(density) if density.is_finite() && density > 0.0 => Ok(density),
        Some(_) => Err(AppError::InvalidInput(
            "Density must be a positive number of grams per mL.".into(),
        )),
    }
}

/// Convert a serving the user typed into the gram equivalent used for nutrients.
pub fn grams_for_serving(amount: f64, unit: &str, density_g_per_ml: Option<f64>) -> Result<f64> {
    if !amount.is_finite() || amount <= 0.0 {
        return Err(AppError::InvalidInput(
            "Amount must be a positive number.".into(),
        ));
    }
    let unit = parse_serving_unit(unit)?;
    let grams = match unit {
        ServingUnit::Gram => amount,
        ServingUnit::Ounce => amount * GRAMS_PER_OUNCE,
        ServingUnit::Milliliter => amount * density_or_default(density_g_per_ml)?,
        ServingUnit::FluidOunce => {
            amount * ML_PER_FLUID_OUNCE * density_or_default(density_g_per_ml)?
        }
    };
    if !grams.is_finite() || grams <= 0.0 {
        return Err(AppError::InvalidInput(
            "That amount does not convert to a positive weight.".into(),
        ));
    }
    Ok(grams)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroTotals {
    pub kcal: f64,
    pub protein: f64,
    pub fat: f64,
    pub carbs: f64,
    pub fiber: f64,
}

pub fn scale_macros(totals: MacroTotals, factor: f64) -> MacroTotals {
    MacroTotals {
        kcal: totals.kcal * factor,
        protein: totals.protein * factor,
        fat: totals.fat * factor,
        carbs: totals.carbs * factor,
        fiber: totals.fiber * factor,
    }
}

/// Nutrients typed for one serving, stored per 100 g so later logs can scale.
pub fn per_100g_from_serving(totals: MacroTotals, grams: f64) -> Result<MacroTotals> {
    if !grams.is_finite() || grams <= 0.0 {
        return Err(AppError::InvalidInput(
            "Cannot store nutrients for a serving that weighs nothing.".into(),
        ));
    }
    Ok(scale_macros(totals, 100.0 / grams))
}

pub fn macros_for_grams(per_100g: MacroTotals, grams: f64) -> Result<MacroTotals> {
    if !grams.is_finite() || grams <= 0.0 {
        return Err(AppError::InvalidInput(
            "Amount must be a positive number.".into(),
        ));
    }
    Ok(scale_macros(per_100g, grams / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) {
        let diff = (actual - expected).abs();
        assert!(
            diff < 1e-6,
            "{actual} is not within 1e-6 of {expected} (diff {diff})"
        );
    }

    #[test]
    fn grams_stay_grams() {
        close(grams_for_serving(100.0, "g", None).unwrap(), 100.0);
        close(grams_for_serving(12.5, "grams", None).unwrap(), 12.5);
    }

    #[test]
    fn milliliters_use_density_and_default_to_water() {
        close(grams_for_serving(250.0, "ml", None).unwrap(), 250.0);
        close(grams_for_serving(250.0, "mL", Some(1.0)).unwrap(), 250.0);
        close(grams_for_serving(15.0, "ml", Some(0.91)).unwrap(), 13.65);
    }

    #[test]
    fn weight_ounces_and_fluid_ounces_are_different() {
        close(
            grams_for_serving(1.0, "oz", None).unwrap(),
            GRAMS_PER_OUNCE,
        );
        close(
            grams_for_serving(1.0, "fl oz", None).unwrap(),
            ML_PER_FLUID_OUNCE,
        );
        close(
            grams_for_serving(8.0, "fl oz", Some(1.03)).unwrap(),
            8.0 * ML_PER_FLUID_OUNCE * 1.03,
        );
        assert!(
            (grams_for_serving(1.0, "oz", None).unwrap()
                - grams_for_serving(1.0, "fl oz", None).unwrap())
            .abs()
                > 1.0
        );
    }

    #[test]
    fn label_nutrients_become_per_100g_then_scale_back() {
        let serving_grams = grams_for_serving(15.0, "ml", Some(0.91)).unwrap();
        let per_100g = per_100g_from_serving(
            MacroTotals {
                kcal: 120.0,
                protein: 0.0,
                fat: 14.0,
                carbs: 0.0,
                fiber: 0.0,
            },
            serving_grams,
        )
        .unwrap();
        close(per_100g.kcal, 120.0 * 100.0 / 13.65);
        let again = macros_for_grams(per_100g, serving_grams).unwrap();
        close(again.kcal, 120.0);
        close(again.fat, 14.0);
    }

    #[test]
    fn hundred_grams_stores_the_typed_nutrients() {
        let grams = grams_for_serving(100.0, "g", None).unwrap();
        let per_100g = per_100g_from_serving(
            MacroTotals {
                kcal: 97.0,
                protein: 10.0,
                fat: 2.0,
                carbs: 3.5,
                fiber: 0.0,
            },
            grams,
        )
        .unwrap();
        close(per_100g.kcal, 97.0);
        close(per_100g.protein, 10.0);
    }

    #[test]
    fn rejects_unknown_units_and_non_positive_amounts() {
        assert!(grams_for_serving(1.0, "cup", None).is_err());
        assert!(grams_for_serving(0.0, "g", None).is_err());
        assert!(grams_for_serving(-5.0, "ml", None).is_err());
        assert!(grams_for_serving(10.0, "ml", Some(0.0)).is_err());
        assert!(grams_for_serving(10.0, "ml", Some(-1.0)).is_err());
    }
}
