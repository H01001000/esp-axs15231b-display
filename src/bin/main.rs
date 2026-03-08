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
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
};
use esp_alloc::ExternalMemory;
use esp_axs15231b_display::{
    axs15231b::{AXS15231B, LcdDisplayBuffer},
    hal::second_core::spawn_on_second_core,
};
use esp_hal::{
    clock::CpuClock,
    psram::{FlashFreq, SpiRamFreq},
};
use esp_hal::{
    gpio::{Level, Output, OutputConfig},
    interrupt::software::SoftwareInterruptControl,
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

// static DRAW_FRAMEBUFFER_BOX: static_cell::StaticCell<Box<[u16; 320 * 480], ExternalMemory>> =
//     static_cell::StaticCell::new();
// static DISPLAY_FRAMEBUFFER_BOX: static_cell::StaticCell<Box<[u16; 320 * 480], ExternalMemory>> =
//     static_cell::StaticCell::new();
static DISPLAY_FRAMEBUFFER_MUTEX: static_cell::StaticCell<
    Mutex<CriticalSectionRawMutex, Box<[u16; 320 * 480], ExternalMemory>>,
> = static_cell::StaticCell::new();

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

pub fn hsv_to_rgb(h: u16, s: f32, v: f32) -> (u8, u8, u8) {
    let c = (v * s) as f32;
    let h_prime = (h as f32) / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let m = v as f32 - c;

    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 | 6 => (c, 0.0, x), // hue could be exactly 360
        _ => (0.0, 0.0, 0.0), // fallback
    };

    let r = ((r1 + m) * 255.0) as u8;
    let g = ((g1 + m) * 255.0) as u8;
    let b = ((b1 + m) * 255.0) as u8;

    (r, g, b)
}

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
            flash_frequency: FlashFreq::FlashFreq80m,
            ram_frequency: SpiRamFreq::Freq80m,
        });

    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(size: 64 * 1024);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
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

    let framebuffer = Box::new_in([0u16; 320 * 480], esp_alloc::ExternalMemory);
    let display_framebuffer = Box::new_in([0u16; 320 * 480], esp_alloc::ExternalMemory);

    let mut lcd_display_buffer = LcdDisplayBuffer::new(framebuffer);

    let display_framebuffer = DISPLAY_FRAMEBUFFER_MUTEX.init(
        Mutex::<CriticalSectionRawMutex, _>::new(display_framebuffer),
    );

    lcd_display_buffer.clear(Rgb565::BLACK);
    lcd_display_buffer.flush(display_framebuffer).await;

    let _ = spawner;

    spawn_on_second_core(
        peripherals.CPU_CTRL,
        sw_int.software_interrupt0,
        sw_int.software_interrupt1,
        |spawner| {
            let lcd_display = AXS15231B::new(
                peripherals.GPIO47,
                peripherals.GPIO21,
                peripherals.GPIO48,
                peripherals.GPIO40,
                peripherals.GPIO39,
                peripherals.GPIO45,
                peripherals.DMA_CH0,
                peripherals.SPI2,
                display_framebuffer,
            );

            spawner.spawn(display_task(lcd_display)).ok();
        },
    );

    let mut backlight = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());

    backlight.set_high();

    let mut x = 0.0;
    let mut x_v = 0.0;
    let x_a = 9.81 * 10.0;
    let mut dt = 0;
    let mut sim_start = Instant::now();
    let mut y = 0;
    let mut hue = 0u16;

    loop {
        let start = Instant::now();

        let draw_start = Instant::now();
        lcd_display_buffer.clear(Rgb565::BLACK);

        let (r, g, b) = hsv_to_rgb(hue, 0.7, 0.7);
        let color = Rgb565::new(r, g, b);

        let circle = Circle::new(Point::new(x as i32, y), 20)
            .into_styled(PrimitiveStyle::with_stroke(color, 1));

        circle.draw(&mut lcd_display_buffer);
        let draw_time = draw_start.elapsed();

        x += x_v * (dt as f32 / 1000.0) + 0.5 * x_a * (dt as f32 / 1000.0) * (dt as f32 / 1000.0);
        x_v += x_a * (dt as f32 / 1000.0);

        if x >= 300.0 {
            x_v = x_v * -0.5;
            x = 300.0;
        }

        y += 1;
        hue = (hue + 1) % 360;

        if sim_start.elapsed() > Duration::from_secs(10) {
            x = 0.0;
            x_v = 0.0;
            y = 0;
            sim_start = Instant::now();
        }

        lcd_display_buffer.flush(display_framebuffer).await;

        dt = start.elapsed().as_millis();

        info!(
            "Draw time: {} ms, total frame time: {} ms",
            draw_time.as_millis(),
            dt
        );
    }
}

#[embassy_executor::task]
pub async fn display_task(mut lcd_display: AXS15231B<'static>) {
    lcd_display.init().await;

    loop {
        lcd_display.flush().await;
    }
}
