#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]
#![feature(allocator_api)]

use alloc::boxed::Box;
use bt_hci::controller::ExternalController;
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
};
use esp_car_dash::axs15231b::LcdDisplay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::{
    clock::CpuClock,
    psram::{FlashFreq, SpiRamFreq},
};
use esp_hal::{
    psram::{PsramConfig, PsramSize},
    timer::timg::TimerGroup,
};
use esp_radio::ble::controller::BleConnector;
use lvgl::{Color, Display, DrawBuffer, TextAlign, style::Style, widgets::Label};
use panic_rtt_target as _;
use trouble_host::prelude::*;

extern crate alloc;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.2.0

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default()
        .with_cpu_clock(CpuClock::max())
        .with_psram(PsramConfig {
            size: PsramSize::Size(8 * 1024 * 1024),
            core_clock: None,
            flash_frequency: FlashFreq::default(),
            ram_frequency: SpiRamFreq::default(),
        });

    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(size: 64 * 1024);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");

    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    let transport = BleConnector::new(&radio_init, peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 1>::new(transport);
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let _stack = trouble_host::new(ble_controller, &mut resources);

    // TODO: Spawn some tasks
    let _ = spawner;

    let mut lcd_display = LcdDisplay::new(
        peripherals.GPIO47,
        peripherals.GPIO21,
        peripherals.GPIO48,
        peripherals.GPIO40,
        peripherals.GPIO39,
        peripherals.GPIO45,
        peripherals.DMA_CH0,
        peripherals.SPI2,
    );

    lcd_display.init().await;

    let mut backlight = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());

    // Initialize the LVGL library. This must be called before any other LVGL functions are used.
    lvgl::init();

    const HOR_RES: u32 = 320;
    const VER_RES: u32 = 480;
    const BUFFER_VER_RES: u32 = 10;

    let buffer = Box::new_in(
        DrawBuffer::<{ (HOR_RES * BUFFER_VER_RES) as usize }>::default(),
        esp_alloc::ExternalMemory,
    );

    let stats: esp_alloc::HeapStats = esp_alloc::HEAP.stats();
    info!("{}", stats);

    // let display = Display::register_raw(draw_buffer, hor_res, ver_res, flush_cb, rounder_cb, set_px_cb, clear_cb, monitor_cb, wait_cb, clean_dcache_cb, drv_update_cb, render_start_cb, drop)

    // Register your display update callback with LVGL. The closure you pass here will be called
    // whenever LVGL has updates to be painted to the display.
    let display = Display::register(*buffer, HOR_RES, VER_RES, |refresh| {
        let start_y = (HOR_RES * (refresh.area.y1 as u32)) as usize;
        let end_y = (HOR_RES * (refresh.area.y2 as u32)) as usize;
        let len_y = (HOR_RES * ((refresh.area.y2 - refresh.area.y1) as u32)) as usize;

        lcd_display.framebuffer[start_y..end_y].copy_from_slice(unsafe {
            core::slice::from_raw_parts(
                refresh.colors[0..len_y].as_ptr() as *const u16,
                refresh.colors[0..len_y].len(),
            )
        });

        // lcd_display.draw_iter(refresh.as_pixels()).unwrap();
    })
    .unwrap();

    backlight.set_high();

    let mut screen = display.get_scr_act().unwrap();
    let mut screen_style = Style::default();
    screen_style.set_bg_color(Color::from_rgb((0, 0, 0)));
    screen_style.set_radius(0);
    lvgl::Widget::add_style(&mut screen, lvgl::Part::Main, &mut screen_style).unwrap();

    let mut time = Label::new().unwrap();
    time.set_text(cstr_core::cstr!("20:46")).unwrap();
    let mut style_time = Style::default();
    style_time.set_text_color(Color::from_rgb((255, 255, 255)));
    style_time.set_text_align(TextAlign::Center);

    lvgl::Widget::add_style(&mut time, lvgl::Part::Main, &mut style_time).unwrap();
    lvgl::Widget::set_width(&mut time, 240).unwrap();
    lvgl::Widget::set_height(&mut time, 240).unwrap();

    let mut y = 0;

    loop {
        lvgl::Widget::set_align(&mut time, lvgl::Align::Center, 0, y).unwrap();

        lvgl::task_handler();
        lcd_display.flush().await;

        y += 1;
        if y > 480 {
            y = 0;
        }

        Timer::after(Duration::from_millis(5)).await;
        lvgl::tick_inc(Duration::from_millis(5).try_into().unwrap());
    }
}
