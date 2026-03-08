#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]
#![feature(allocator_api)]

use bt_hci::controller::ExternalController;
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant};
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
    lcd_display.clear(Rgb565::BLACK);
    lcd_display.flush().await;

    let mut backlight = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());

    backlight.set_high();

    let mut x = 0.0;
    let mut x_v = 0.0;
    let x_a = 9.81 * 10.0;
    let mut dt = 0;
    let mut sim_start = Instant::now();
    let mut y = 0;
    let mut color = 0u16;

    loop {
        let start = Instant::now();
        lcd_display.clear(Rgb565::BLACK);

        let circle = Circle::new(Point::new(x as i32, y), 20)
            .into_styled(PrimitiveStyle::with_stroke(Rgb565::new(255, 255, 255), 1));

        circle.draw(&mut lcd_display);
        let draw_time = start.elapsed();

        // Update the display
        let flush_start = Instant::now();
        lcd_display.flush().await;
        let flush_time = flush_start.elapsed();

        x += x_v * (dt as f32 / 1000.0) + 0.5 * x_a * (dt as f32 / 1000.0) * (dt as f32 / 1000.0);
        x_v += x_a * (dt as f32 / 1000.0);

        if x >= 300.0 {
            x_v = x_v * -0.5;
            x = 300.0;
        }

        y += 1;

        if sim_start.elapsed() > Duration::from_secs(10) {
            x = 0.0;
            x_v = 0.0;
            y = 0;
            sim_start = Instant::now();
            // lcd_display.clear(Rgb565::BLACK);
        }

        dt = start.elapsed().as_millis();

        info!(
            "Draw time: {} ms, Flush time: {} ms, total frame time: {} ms",
            draw_time.as_millis(),
            flush_time.as_millis(),
            dt
        );
        // Timer::after(Duration::from_millis(5)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples
}
