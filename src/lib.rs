use image::{
    png::{CompressionType, FilterType, PngEncoder},
    ColorType,
};
use worker::*;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{
        Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
    },
    text::{Alignment, Text},
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
) -> std::result::Result<(), <SimulatorDisplay<Rgb888> as DrawTarget>::Error> {
    display.clear(Rgb888::WHITE)?;
    // Create styles used by the drawing operations.
    let thin_stroke = PrimitiveStyle::with_stroke(Rgb888::BLACK, 1);
    let thick_stroke = PrimitiveStyle::with_stroke(Rgb888::BLACK, 3);
    let border_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb888::BLACK)
        .stroke_width(3)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();
    let fill = PrimitiveStyle::with_fill(Rgb888::BLACK);
    let character_style = MonoTextStyle::new(&FONT_6X10, Rgb888::BLACK);

    let yoffset = 14;

    // Draw a 3px wide outline around the display.
    display
        .bounding_box()
        .into_styled(border_stroke)
        .draw(display)?;

    // Draw a triangle.
    Triangle::new(
        Point::new(16, 16 + yoffset),
        Point::new(16 + 16, 16 + yoffset),
        Point::new(16 + 8, yoffset),
    )
    .into_styled(thin_stroke)
    .draw(display)?;

    // Draw a filled square
    Rectangle::new(Point::new(52, yoffset), Size::new(16, 16))
        .into_styled(fill)
        .draw(display)?;

    // Draw a circle with a 3px wide stroke.
    Circle::new(Point::new(88, yoffset), 17)
        .into_styled(thick_stroke)
        .draw(display)?;

    // Draw centered text.
    let text = "embedded-graphics";
    Text::with_alignment(
        text,
        display.bounding_box().center() + Point::new(0, 15),
        character_style,
        Alignment::Center,
    )
    .draw(display)?;

    Ok(())
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
        .get("/image", |_req, _ctx| {
            // Create a new simulator display with 128x64 pixels.
            let mut display: SimulatorDisplay<Rgb888> = SimulatorDisplay::new(Size::new(296, 128));
            if let Err(_err) = render(&mut display) {
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
