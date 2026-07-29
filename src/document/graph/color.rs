use anyhow::{Result, anyhow, bail};

use super::types::GraphColor;

pub(super) fn parse_color(source: &str) -> Result<GraphColor> {
    let source = source.trim();
    if let Some(hex) = source.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(args) = function_args(source, "rgb").or_else(|| function_args(source, "rgba")) {
        return parse_rgb(args);
    }
    if let Some(args) = function_args(source, "hsl").or_else(|| function_args(source, "hsla")) {
        return parse_hsl(args);
    }
    if let Some(args) = function_args(source, "oklch") {
        return parse_oklch(args);
    }
    bail!("unsupported color {source:?}")
}

fn function_args<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    source
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
}

fn parse_hex(hex: &str) -> Result<GraphColor> {
    let expanded = match hex.len() {
        3 => format!(
            "{}{}{}{}{}{}ff",
            &hex[0..1],
            &hex[0..1],
            &hex[1..2],
            &hex[1..2],
            &hex[2..3],
            &hex[2..3]
        ),
        6 => format!("{hex}ff"),
        8 => hex.to_string(),
        _ => bail!("hex colors must use #RGB, #RRGGBB, or #RRGGBBAA"),
    };
    let bytes = (0..4)
        .map(|i| u8::from_str_radix(&expanded[i * 2..i * 2 + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(GraphColor {
        red: bytes[0] as f32 / 255.0,
        green: bytes[1] as f32 / 255.0,
        blue: bytes[2] as f32 / 255.0,
        alpha: bytes[3] as f32 / 255.0,
    })
}

fn color_parts(args: &str) -> Vec<String> {
    args.replace(',', " ")
        .split_whitespace()
        .filter(|part| *part != "/")
        .map(str::to_string)
        .collect()
}

fn parse_rgb(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("rgb requires three channels and optional alpha");
    }
    let channel = |part: &str| -> Result<f32> {
        if let Some(percent) = part.strip_suffix('%') {
            Ok(percent.parse::<f32>()? / 100.0)
        } else {
            Ok(part.parse::<f32>()? / 255.0)
        }
    };
    make_color(
        channel(&parts[0])?,
        channel(&parts[1])?,
        channel(&parts[2])?,
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

fn parse_hsl(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("hsl requires hue, saturation, lightness, and optional alpha");
    }
    let h = parts[0]
        .trim_end_matches("deg")
        .parse::<f32>()?
        .rem_euclid(360.0)
        / 360.0;
    let s = percentage(&parts[1])?;
    let l = percentage(&parts[2])?;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        if t < 0.0 {
            t += 1.0
        }
        if t > 1.0 {
            t -= 1.0
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    make_color(
        hue(h + 1.0 / 3.0),
        hue(h),
        hue(h - 1.0 / 3.0),
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

#[allow(clippy::excessive_precision)] // Published OKLab conversion coefficients.
fn parse_oklch(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("oklch requires lightness, chroma, hue, and optional alpha");
    }
    let l = if parts[0].ends_with('%') {
        percentage(&parts[0])?
    } else {
        parts[0].parse()?
    };
    let c: f32 = parts[1].parse()?;
    let h = parts[2]
        .trim_end_matches("deg")
        .parse::<f32>()?
        .to_radians();
    if !(0.0..=1.0).contains(&l) || c < 0.0 || !c.is_finite() {
        bail!("oklch lightness or chroma is outside its valid range");
    }
    let a = c * h.cos();
    let b = c * h.sin();
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.291485548 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    let linear = [
        4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
        -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
        -0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3,
    ];
    let gamma = |v: f32| {
        if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    // CSS maps out-of-gamut OKLCH colors into the output gamut. A channel
    // clamp is deterministic and adequate for the native sRGB renderer.
    make_color(
        gamma(linear[0]).clamp(0.0, 1.0),
        gamma(linear[1]).clamp(0.0, 1.0),
        gamma(linear[2]).clamp(0.0, 1.0),
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

fn percentage(value: &str) -> Result<f32> {
    Ok(value
        .strip_suffix('%')
        .ok_or_else(|| anyhow!("expected percentage"))?
        .parse::<f32>()?
        / 100.0)
}

fn alpha(value: &str) -> Result<f32> {
    if value.ends_with('%') {
        percentage(value)
    } else {
        Ok(value.parse()?)
    }
}

fn make_color(red: f32, green: f32, blue: f32, alpha: f32) -> Result<GraphColor> {
    if [red, green, blue, alpha]
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        bail!("color channel is outside its valid range");
    }
    Ok(GraphColor {
        red,
        green,
        blue,
        alpha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_color_variants() {
        assert_eq!(parse_color("#00000000").unwrap().alpha, 0.0);
        assert!(parse_color("rgb(300 0 0)").is_err());
    }
}
