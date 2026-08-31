/// A color as (red, green, blue, alpha), each 0..=255.
type Rgba = (u8, u8, u8, u8);

#[derive(Debug)]
struct Params {
    /// Number of integers to test for primality (2..=n).
    n: u64,
    /// Image width in pixels.
    width: u32,
    /// Image height in pixels.
    height: u32,
    /// Absolute radius scale: pixels per unit of the integer value.
    /// scale <= 0 means "size relative to the image via --fill" instead.
    scale: f64,
    /// Relative image scaling, independent of --n: the largest prime spans
    /// `fill` times the image radius. Only used while scale <= 0.
    /// fill > 1 deliberately pushes the outer primes off the image (crop).
    fill: f64,
    /// Dot radius in pixels.
    dot_radius: f64,
    /// Angular step per integer, in radians.
    /// 1.0 = classic Sacks spiral (prime p at angle p rad, radius p*scale).
    angle_step: f64,
    /// Point color.
    color: Rgba,
    /// Shift of the spiral center along the x axis, as a fraction of the
    /// half-image width. 0.5 = half of the half-width to the right,
    /// -1.0 = one half-width to the left. 0 = image center.
    offset_x: f64,
    /// Shift of the spiral center along the y axis, as a fraction of the
    /// half-image height. Positive = down, negative = up. 0 = image center.
    offset_y: f64,
    /// Output file name; empty = auto "image_YYYYMMDD.png".
    output: String,
}

impl Params {
    /// Parse CLI arguments:
    /// --n --width --height --scale --fill --dot-radius --angle-step --color --output
    fn parse(args: &[String]) -> Result<Params, String> {
        let mut p = Params {
            n: 1000,
            width: 1000,
            height: 1000,
            scale: 0.0, // 0 = relative sizing via --fill
            fill: 1.0,
            dot_radius: 1.5,
            angle_step: 1.0,
            color: (255, 255, 255, 255),
            offset_x: 0.0,
            offset_y: 0.0,
            output: String::new(),
        };
        let mut i = 0;
        while i < args.len() {
            let val = |i: usize| args.get(i + 1).ok_or(format!("missing value for {}", args[i]));
            match args[i].as_str() {
                "--n" => p.n = val(i)?.parse().map_err(|_| "invalid --n")?,
                "--width" => p.width = val(i)?.parse().map_err(|_| "invalid --width")?,
                "--height" => p.height = val(i)?.parse().map_err(|_| "invalid --height")?,
                "--scale" => p.scale = val(i)?.parse().map_err(|_| "invalid --scale")?,
                "--fill" => p.fill = val(i)?.parse().map_err(|_| "invalid --fill")?,
                "--dot-radius" => p.dot_radius = val(i)?.parse().map_err(|_| "invalid --dot-radius")?,
                "--angle-step" => p.angle_step = val(i)?.parse().map_err(|_| "invalid --angle-step")?,
                "--color" => p.color = parse_color(val(i)?)?,
                "--offset-x" => p.offset_x = val(i)?.parse().map_err(|_| "invalid --offset-x")?,
                "--offset-y" => p.offset_y = val(i)?.parse().map_err(|_| "invalid --offset-y")?,
                "--output" => p.output = val(i)?.clone(),
                _ => return Err(format!("unknown argument: {}", args[i])),
            }
            i += 2;
        }
        if p.width == 0 || p.height == 0 {
            return Err("--width/--height must be >= 1".into());
        }
        if p.dot_radius <= 0.0 {
            return Err("--dot-radius must be > 0".into());
        }
        if p.angle_step == 0.0 {
            return Err("--angle-step must be != 0".into());
        }
        if p.fill <= 0.0 {
            return Err("--fill must be > 0".into());
        }
        Ok(p)
    }
}

/// Parse a color given as "#RRGGBB", "RRGGBB", "#RRGGBBAA", "RRGGBBAA"
/// or one of a few common names. Returns (r, g, b, a).
fn parse_color(s: &str) -> Result<Rgba, String> {
    if let Some((r, g, b)) = named_color(&s.to_ascii_lowercase()) {
        return Ok((r, g, b, 255));
    }
    let hex = s.strip_prefix('#').unwrap_or(s);
    // is_ascii guards the byte slicing below against multi-byte characters.
    if !hex.is_ascii() || (hex.len() != 6 && hex.len() != 8) {
        return Err(format!(
            "invalid color: {} (use #RRGGBB, #RRGGBBAA or a named color)",
            s
        ));
    }
    let byte = |h: &str| u8::from_str_radix(h, 16).map_err(|_| format!("invalid color: {}", s));
    let r = byte(&hex[0..2])?;
    let g = byte(&hex[2..4])?;
    let b = byte(&hex[4..6])?;
    let a = if hex.len() == 8 { byte(&hex[6..8])? } else { 255 };
    Ok((r, g, b, a))
}

/// A few common color names (CSS values).
fn named_color(name: &str) -> Option<(u8, u8, u8)> {
    Some(match name {
        "white" => (255, 255, 255),
        "black" => (0, 0, 0),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "lime" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" => (0, 255, 255),
        "magenta" => (255, 0, 255),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "pink" => (255, 192, 203),
        "gray" | "grey" => (128, 128, 128),
        _ => return None,
    })
}

/// Simple sieve of Eratosthenes.
fn sieve(n: u64) -> Vec<u64> {
    if n < 2 {
        return vec![];
    }
    let mut is_composite = vec![false; (n + 1) as usize];
    let mut primes = Vec::new();
    for i in 2..=n {
        if !is_composite[i as usize] {
            primes.push(i);
            let mut j = i * i;
            while j <= n {
                is_composite[j as usize] = true;
                j += i;
            }
        }
    }
    primes
}

/// Render the polar plot of primes onto an RGBA buffer (opaque black background).
/// Returns the buffer and the effective radius scale used.
fn render(p: &Params, primes: &[u64]) -> (Vec<u8>, f64) {
    let w = p.width as usize;
    let h = p.height as usize;
    let mut buf = vec![0u8; w * h * 4];
    for px in buf.chunks_exact_mut(4) {
        px[3] = 255; // opaque background
    }
    let max_r = ((w.min(h) as f64 / 2.0) - p.dot_radius - 1.0).max(0.0);

    // Absolute scale wins; otherwise size the spiral relative to the image,
    // independent of --n: the largest prime always lands at fill * max_r,
    // so changing n only changes point density, never the spiral's extent.
    let scale = if p.scale > 0.0 {
        p.scale
    } else {
        match primes.last() {
            Some(&last) if max_r > 0.0 => max_r * p.fill / last as f64,
            _ => return (buf, 0.0), // no primes or degenerate image -> empty
        }
    };

    // Offsets are given as a fraction of the half-image (0.5 = half of the
    // half-width to the right, -1.0 = one half-height up). This keeps the
    // shift proportional across image sizes and --n values, and makes
    // fractional values meaningful.
    let cx = w as f64 / 2.0 + p.offset_x * w as f64 / 2.0;
    let cy = h as f64 / 2.0 + p.offset_y * h as f64 / 2.0;

    for &prime in primes {
        let angle = prime as f64 * p.angle_step;
        let r = prime as f64 * scale;
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        draw_dot(&mut buf, w, h, x, y, p.dot_radius, p.color);
    }
    (buf, scale)
}

/// Draw a filled circle at (x, y) with the given radius and color. Clips to image bounds.
/// The color's alpha is blended source-over so the buffer stays fully opaque.
fn draw_dot(buf: &mut [u8], w: usize, h: usize, x: f64, y: f64, radius: f64, color: Rgba) {
    let x0 = (x - radius).floor().max(0.0) as usize;
    let x1 = ((x + radius).ceil() as i64).min(w as i64 - 1).max(0) as usize;
    let y0 = (y - radius).floor().max(0.0) as usize;
    let y1 = ((y + radius).ceil() as i64).min(h as i64 - 1).max(0) as usize;
    let r2 = radius * radius;
    let (cr, cg, cb, ca) = color;
    let a = ca as f64 / 255.0;
    let inv = 1.0 - a;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = px as f64 - x;
            let dy = py as f64 - y;
            if dx * dx + dy * dy <= r2 {
                let idx = (py * w + px) * 4;
                buf[idx] = (cr as f64 * a + buf[idx] as f64 * inv) as u8;
                buf[idx + 1] = (cg as f64 * a + buf[idx + 1] as f64 * inv) as u8;
                buf[idx + 2] = (cb as f64 * a + buf[idx + 2] as f64 * inv) as u8;
            }
        }
    }
}

/// Write an RGBA buffer as a PNG file.
fn write_png(path: &str, buf: &[u8], w: u32, h: u32) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut wtr = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(&mut wtr, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(buf).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let p = match Params::parse(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Usage: polar_primes [--n <int>] [--width <int>] [--height <int>]");
            eprintln!("                  [--scale <float>] [--fill <float>] [--dot-radius <float>]");
            eprintln!("                  [--angle-step <float>] [--color <color>]");
            eprintln!("                  [--offset-x <float>] [--offset-y <float>] [--output <file>]");
            eprintln!("  --color: #RRGGBB, #RRGGBBAA or a name (white black red green lime blue");
            eprintln!("           yellow cyan magenta orange purple pink gray)");
            eprintln!("  --scale: absolute px per integer (overrides --fill)");
            eprintln!("  --fill:  fraction of the image the spiral spans, independent of --n");
            eprintln!("  --offset-x/--offset-y: shift the spiral center as a fraction of the");
            eprintln!("           half-image (0.5 = half of the half-width; positive x = right,");
            eprintln!("           positive y = down; negative values allowed)");
            eprintln!("Defaults: n=1000 width=1000 height=1000 scale=auto fill=1.0");
            eprintln!("          dot-radius=1.5 angle-step=1.0 color=white");
            std::process::exit(1);
        }
    };
    let primes = sieve(p.n);
    println!("Found {} primes up to {}", primes.len(), p.n);
    let (buf, scale) = render(&p, &primes);
    println!(
        "Radius scale: {:.4} px per integer, point color #{:02x}{:02x}{:02x}",
        scale, p.color.0, p.color.1, p.color.2
    );
    // Show the effective pixel shift so a too-small or too-large offset is
    // immediately visible in the console.
    println!(
        "Offset: ({:+.3}, {:+.3}) of half-image = ({:+.1}, {:+.1}) px",
        p.offset_x,
        p.offset_y,
        p.offset_x * p.width as f64 / 2.0,
        p.offset_y * p.height as f64 / 2.0
    );
    let path = if p.output.is_empty() {
        format!("image_{}.png", chrono::Local::now().format("%Y%m%d"))
    } else {
        p.output.clone()
    };
    match write_png(&path, &buf, p.width, p.height) {
        Ok(_) => println!("Wrote {} ({}x{})", path, p.width, p.height),
        Err(e) => {
            eprintln!("Error writing PNG: {}", e);
            std::process::exit(1);
        }
    }
}