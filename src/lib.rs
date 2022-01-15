use image::{
    png::{CompressionType, FilterType, PngEncoder},
    ColorType,
};
use serde::Deserialize;
use std::convert::TryInto;
use worker::*;

use embedded_graphics::{
    mono_font::{
        ascii::{FONT_10X20, FONT_7X13_BOLD},
        MonoTextStyle,
    },
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Baseline, Text},
};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay};

mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
}

fn render(
    display: &mut SimulatorDisplay<Rgb888>,
    quote: &Quote,
) -> std::result::Result<(), <SimulatorDisplay<Rgb888> as DrawTarget>::Error> {
    display.clear(Rgb888::WHITE)?;
    let dw: i32 = display.size().width.try_into().unwrap();
    let dh: i32 = display.size().height.try_into().unwrap();
    let highlight = PrimitiveStyle::with_fill(Rgb888::RED);

    // Header
    let head_h: i32 = dh / 4;
    let head_font = MonoTextStyle::new(&FONT_10X20, Rgb888::WHITE);
    let head_font_h: i32 = 20;
    Rectangle::new(
        Point::new(0, 0),
        Size::new(dw.try_into().unwrap(), head_h.try_into().unwrap()),
    )
    .into_styled(highlight)
    .draw(display)?;
    Text::with_alignment(
        "Quote of the Day",
        Point::new(
            display.bounding_box().center().x,
            (head_font_h).try_into().unwrap(),
        ),
        head_font,
        Alignment::Center,
    )
    .draw(display)?;

    // Quote
    let quote_author_font = MonoTextStyle::new(&FONT_7X13_BOLD, Rgb888::BLACK);
    let quote_font = MonoTextStyle::new(&FONT_10X20, Rgb888::BLACK);
    let quote_author_font_h: i32 = 7;
    let quote_font_w: i32 = 10;
    let quote_font_h: i32 = 14;
    let pad: i32 = 3;
    Text::with_baseline(
        quote.a.as_str(),
        Point::new(pad, head_h + pad),
        quote_author_font,
        Baseline::Top,
    )
    .draw(display)?;

    // wrapping the text
    let words = quote.q.split_ascii_whitespace();
    let max_line: usize = ((dw - 2 * pad) / quote_font_w).try_into().unwrap();
    let mut line: Vec<&str> = vec![];
    let mut line_len = 0;
    let mut total_lines = 0;
    let end_marker = "<end>";
    for word in words.chain(vec![end_marker]) {
        if line_len != 0 && line_len + word.len() > max_line || word == end_marker {
            Text::with_baseline(
                &line.join(" "),
                Point::new(
                    pad,
                    head_h
                        + pad
                        + quote_author_font_h
                        + 3*pad
                        + pad * total_lines
                        + quote_font_h * total_lines,
                ),
                quote_font,
                Baseline::Top,
            )
            .draw(display)?;
            total_lines += 1;
            line.truncate(0);
            line_len = 0;
        }
        line.push(word);
        line_len += word.len() + 1;
    }

    Ok(())
}

#[derive(Deserialize)]
pub struct Quote {
    q: String,
    a: String,
}

#[event(fetch)]
pub async fn main(req: Request, env: Env) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many routes as your Worker needs! Each route will get a `Request` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .get("/", |_, _| Response::ok("Hello from Workers!"))
        .get_async("/image", |_req, _ctx| async move {
            // Create a new simulator display with 128x64 pixels.
            let mut display: SimulatorDisplay<Rgb888> = SimulatorDisplay::new(Size::new(296, 128));

            let quote_resp = Fetch::Url(Url::parse("https://zenquotes.io/api/today").unwrap())
                .send()
                .await;
            if let Err(err) = quote_resp {
                return Response::error(format!("no quotes available: {:?}", err), 503);
            }
            let mut quote_resp = quote_resp.unwrap();
            // API is stupid...
            let _ = quote_resp
                .headers_mut()
                .set("content-type", "application/json");
            let quote_resp = quote_resp.json().await;
            if let Err(err) = quote_resp {
                return Response::error(format!("no JSON quotes available: {:?}", err), 503);
            }
            let quotes: Vec<Quote> = quote_resp.unwrap();

            if let Err(_err) = render(&mut display, quotes.get(0).unwrap()) {
                return Response::error("Bad Request", 400);
            }

            let output_settings = OutputSettingsBuilder::new().build();
            let out_img = display.to_rgb_output_image(&output_settings);
            let img_buf = out_img.as_image_buffer();
            let mut png = Vec::new();
            let encoded = PngEncoder::new_with_quality(
                &mut png,
                CompressionType::Best,
                FilterType::default(),
            )
            .encode(
                &img_buf,
                display.size().width,
                display.size().height,
                ColorType::Rgb8,
            );

            if let Err(err) = encoded {
                return Response::error(
                    format!("ups: {:?}, buf dim: {:?}", err, img_buf.dimensions()),
                    500,
                );
            }

            let mut headers = Headers::new();
            headers.set("Content-Type", "image/png")?;
            Ok(Response::from_bytes(png).unwrap().with_headers(headers))
        })
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .run(req, env)
        .await
}
