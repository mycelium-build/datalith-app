use anyhow::{Context, Result, anyhow, bail};

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
        3 => {
            let mut expanded = String::with_capacity(8);
            for byte in hex.as_bytes() {
                expanded.push(char::from(*byte));
                expanded.push(char::from(*byte));
            }
            expanded.push_str("ff");
            expanded
        }
        6 => format!("{hex}ff"),
        8 => hex.to_string(),
        _ => bail!("hex colors must use #RGB, #RRGGBB, or #RRGGBBAA"),
    };
    let bytes = expanded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| -> Result<u8> {
            let pair = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(pair, 16)?)
        })
        .collect::<Result<Vec<_>>>()
        .with_context(|| format!("invalid hex color #{hex}"))?;
    let [red, green, blue, alpha] = bytes.as_slice() else {
        bail!("hex color must expand to four channels");
    };
    Ok(GraphColor {
        red: f32::from(*red) / 255.0,
        green: f32::from(*green) / 255.0,
        blue: f32::from(*blue) / 255.0,
        alpha: f32::from(*alpha) / 255.0,
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
    let [red, green, blue] = parts.as_slice() else {
        bail!("rgb requires three channels");
    };
    make_color(
        channel(red)?,
        channel(green)?,
        channel(blue)?,
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

fn parse_hsl(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("hsl requires hue, saturation, lightness, and optional alpha");
    }
    let [hue, saturation, lightness] = parts.as_slice() else {
        bail!("hsl requires hue, saturation, and lightness channels");
    };
    let h = hue
        .trim_end_matches("deg")
        .parse::<f32>()?
        .rem_euclid(360.0)
        / 360.0;
    let sat = percentage(saturation)?;
    let light = percentage(lightness)?;
    let q = if light < 0.5 {
        light * (1.0 + sat)
    } else {
        light.mul_add(-sat, light + sat)
    };
    let p = 2.0f32.mul_add(light, -q);
    let hue_fn = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            ((q - p) * 6.0).mul_add(t, p)
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            ((q - p) * (2.0 / 3.0 - t)).mul_add(6.0, p)
        } else {
            p
        }
    };
    make_color(
        hue_fn(h + 1.0 / 3.0),
        hue_fn(h),
        hue_fn(h - 1.0 / 3.0),
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

#[allow(clippy::excessive_precision, clippy::suboptimal_flops)]
// Published OKLab conversion coefficients; nested mul_add forms would obscure the reference.
fn parse_oklch(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("oklch requires lightness, chroma, hue, and optional alpha");
    }
    let [lightness, chroma, hue] = parts.as_slice() else {
        bail!("oklch requires lightness, chroma, and hue channels");
    };
    let light = if lightness.ends_with('%') {
        percentage(lightness)?
    } else {
        lightness.parse::<f32>()?
    };
    let chroma: f32 = chroma.parse()?;
    let hue: f32 = hue.trim_end_matches("deg").parse::<f32>()?.to_radians();
    if !(0.0..=1.0).contains(&light) || chroma < 0.0 || !chroma.is_finite() {
        bail!("oklch lightness or chroma is outside its valid range");
    }
    let a = chroma * hue.cos();
    let b = chroma * hue.sin();
    let l_ = light + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = light - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = light - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    let [red_linear, green_linear, blue_linear] = [
        4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3,
        -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701 * s3,
    ];
    let gamma = |v: f32| {
        if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    // CSS maps out-of-gamut OKLCH colors into the output gamut. A channel
    // clamp is deterministic and adequate for the native sRGB renderer.
    make_color(
        gamma(red_linear).clamp(0.0, 1.0),
        gamma(green_linear).clamp(0.0, 1.0),
        gamma(blue_linear).clamp(0.0, 1.0),
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
        let color = parse_color("#00000000").unwrap();
        assert!((color.alpha - 0.0).abs() <= 1e-6);
        assert!(parse_color("rgb(300 0 0)").is_err());
    }
}
