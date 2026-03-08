use alloc::boxed::Box;
use core::convert::TryInto;
use defmt::info;
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use esp_hal::{
    dma::{DmaChannelFor, DmaRxBuf, DmaTxBuf},
    dma_buffers,
    gpio::interconnect::{PeripheralInput, PeripheralOutput},
    spi::{
        Mode,
        master::{Address, AnySpi, Command, Config, DataMode, Instance, Spi},
    },
    time::Rate,
};

extern crate alloc;

pub struct LcdCommand<'a> {
    pub cmd: u32,
    pub data: &'a [u8],
    pub delay_ms: u32,
}

pub const LCD_INIT_TABLE: &[LcdCommand] = &[
    LcdCommand {
        cmd: 0xBB,
        data: &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x5A, 0xA],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xA0,
        data: &[
            0xC0, 0x10, 0x00, 0x02, 0x00, 0x00, 0x04, 0x3F, 0x20, 0x05, 0x3F, 0x3F, 0x00, 0x00,
            0x00, 0x00, 0x00,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xA2,
        data: &[
            0x30, 0x3C, 0x24, 0x14, 0xD0, 0x20, 0xFF, 0xE0, 0x40, 0x19, 0x80, 0x80, 0x80, 0x20,
            0xf9, 0x10, 0x02, 0xff, 0xff, 0xF0, 0x90, 0x01, 0x32, 0xA0, 0x91, 0xE0, 0x20, 0x7F,
            0xFF, 0x00, 0x5A,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xD0,
        data: &[
            0xE0, 0x40, 0x51, 0x24, 0x08, 0x05, 0x10, 0x01, 0x20, 0x15, 0x42, 0xC2, 0x22, 0x22,
            0xAA, 0x03, 0x10, 0x12, 0x60, 0x14, 0x1E, 0x51, 0x15, 0x00, 0x8A, 0x20, 0x00, 0x03,
            0x3A, 0x12,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xA3,
        data: &[
            0xA0, 0x06, 0xAa, 0x00, 0x08, 0x02, 0x0A, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
            0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x55, 0x55,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xC1,
        data: &[
            0x31, 0x04, 0x02, 0x02, 0x71, 0x05, 0x24, 0x55, 0x02, 0x00, 0x41, 0x00, 0x53, 0xFF,
            0xFF, 0xFF, 0x4F, 0x52, 0x00, 0x4F, 0x52, 0x00, 0x45, 0x3B, 0x0B, 0x02, 0x0d, 0x00,
            0xFF, 0x40,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xC3,
        data: &[
            0x00, 0x00, 0x00, 0x50, 0x03, 0x00, 0x00, 0x00, 0x01, 0x80, 0x01,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xC4,
        data: &[
            0x00, 0x24, 0x33, 0x80, 0x00, 0xea, 0x64, 0x32, 0xC8, 0x64, 0xC8, 0x32, 0x90, 0x90,
            0x11, 0x06, 0xDC, 0xFA, 0x00, 0x00, 0x80, 0xFE, 0x10, 0x10, 0x00, 0x0A, 0x0A, 0x44,
            0x50,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xC5,
        data: &[
            0x18, 0x00, 0x00, 0x03, 0xFE, 0x3A, 0x4A, 0x20, 0x30, 0x10, 0x88, 0xDE, 0x0D, 0x08,
            0x0F, 0x0F, 0x01, 0x3A, 0x4A, 0x20, 0x10, 0x10, 0x00,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xC6,
        data: &[
            0x05, 0x0A, 0x05, 0x0A, 0x00, 0xE0, 0x2E, 0x0B, 0x12, 0x22, 0x12, 0x22, 0x01, 0x03,
            0x00, 0x3F, 0x6A, 0x18, 0xC8, 0x22,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xC7,
        data: &[
            0x50, 0x32, 0x28, 0x00, 0xa2, 0x80, 0x8f, 0x00, 0x80, 0xff, 0x07, 0x11, 0x9c, 0x67,
            0xff, 0x24, 0x0c, 0x0d, 0x0e, 0x0f,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xC9,
        data: &[0x33, 0x44, 0x44, 0x0],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xCF,
        data: &[
            0x2C, 0x1E, 0x88, 0x58, 0x13, 0x18, 0x56, 0x18, 0x1E, 0x68, 0x88, 0x00, 0x65, 0x09,
            0x22, 0xC4, 0x0C, 0x77, 0x22, 0x44, 0xAA, 0x55, 0x08, 0x08, 0x12, 0xA0, 0x08,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xD5,
        data: &[
            0x40, 0x8E, 0x8D, 0x01, 0x35, 0x04, 0x92, 0x74, 0x04, 0x92, 0x74, 0x04, 0x08, 0x6A,
            0x04, 0x46, 0x03, 0x03, 0x03, 0x03, 0x82, 0x01, 0x03, 0x00, 0xE0, 0x51, 0xA1, 0x00,
            0x00, 0x00,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xD6,
        data: &[
            0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE, 0x93, 0x00, 0x01, 0x83, 0x07, 0x07,
            0x00, 0x07, 0x07, 0x00, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x00, 0x84, 0x00, 0x20,
            0x01, 0x00,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xD7,
        data: &[
            0x03, 0x01, 0x0b, 0x09, 0x0f, 0x0d, 0x1E, 0x1F, 0x18, 0x1d, 0x1f, 0x19, 0x40, 0x8E,
            0x04, 0x00, 0x20, 0xA0, 0x1F,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xD8,
        data: &[
            0x02, 0x00, 0x0a, 0x08, 0x0e, 0x0c, 0x1E, 0x1F, 0x18, 0x1d, 0x1f, 0x19,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xD9,
        data: &[
            0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xDD,
        data: &[
            0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xDF,
        data: &[0x44, 0x73, 0x4B, 0x69, 0x00, 0x0A, 0x02, 0x9],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xE0,
        data: &[
            0x3B, 0x28, 0x10, 0x16, 0x0c, 0x06, 0x11, 0x28, 0x5c, 0x21, 0x0D, 0x35, 0x13, 0x2C,
            0x33, 0x28, 0x0D,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xE1,
        data: &[
            0x37, 0x28, 0x10, 0x16, 0x0b, 0x06, 0x11, 0x28, 0x5C, 0x21, 0x0D, 0x35, 0x14, 0x2C,
            0x33, 0x28, 0x0F,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xE2,
        data: &[
            0x3B, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x35, 0x44, 0x32, 0x0C, 0x14, 0x14, 0x36,
            0x3A, 0x2F, 0x0D,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xE3,
        data: &[
            0x37, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x35, 0x44, 0x32, 0x0C, 0x14, 0x14, 0x36,
            0x32, 0x2F, 0x0F,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xE4,
        data: &[
            0x3B, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x39, 0x44, 0x2E, 0x0C, 0x14, 0x14, 0x36,
            0x3A, 0x2F, 0x0D,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xE5,
        data: &[
            0x37, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x39, 0x44, 0x2E, 0x0C, 0x14, 0x14, 0x36,
            0x3A, 0x2F, 0x0F,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xA4,
        data: &[
            0x85, 0x85, 0x95, 0x82, 0xAF, 0xAA, 0xAA, 0x80, 0x10, 0x30, 0x40, 0x40, 0x20, 0xFF,
            0x60, 0x30,
        ],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xA4,
        data: &[0x85, 0x85, 0x95, 0x8],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0xBB,
        data: &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0x13,
        data: &[0x0],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0x11,
        data: &[0x0],
        delay_ms: 120,
    },
    LcdCommand {
        cmd: 0x2C,
        data: &[0x00, 0x00, 0x00, 0x0],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0x2a,
        data: &[0x00, 0x00, 0x01, 0x3],
        delay_ms: 0,
    },
    LcdCommand {
        cmd: 0x2b,
        data: &[0x00, 0x00, 0x01, 0xd],
        delay_ms: 0,
    },
];

pub const LCD_OPCODE_WRITE_CMD: u16 = 0x02;
pub const LCD_OPCODE_READ_CMD: u16 = 0x0B;
pub const LCD_OPCODE_WRITE_COLOR: u16 = 0x32;

pub const LCD_WIDTH: u16 = 320;
pub const LCD_HEIGHT: u16 = 480;

pub struct LcdDisplay<'a> {
    pub framebuffer:
        Box<[u16; LCD_WIDTH as usize * LCD_HEIGHT as usize], esp_alloc::ExternalMemory>,
    spi: esp_hal::spi::master::SpiDmaBus<'a, esp_hal::Async>,
    // iface: SPI1,
}

impl<'a> LcdDisplay<'a> {
    pub fn new(
        sck: impl PeripheralOutput<'a>,
        sio0: impl PeripheralInput<'a> + PeripheralOutput<'a>,
        sio1: impl PeripheralInput<'a> + PeripheralOutput<'a>,
        sio2: impl PeripheralInput<'a> + PeripheralOutput<'a>,
        sio3: impl PeripheralInput<'a> + PeripheralOutput<'a>,
        cs: impl PeripheralOutput<'a>,
        dma: impl DmaChannelFor<AnySpi<'a>>,
        spi: impl Instance + 'a,
    ) -> Self {
        let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(4096, 4096 * 8);
        let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
        let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

        Self {
            spi: Spi::new(
                spi,
                Config::default()
                    .with_frequency(Rate::from_mhz(60))
                    .with_mode(Mode::_3),
            )
            .unwrap()
            .with_sck(sck)
            .with_sio0(sio0)
            .with_sio1(sio1)
            .with_sio2(sio2)
            .with_sio3(sio3)
            .with_cs(cs)
            .with_dma(dma)
            .with_buffers(dma_rx_buf, dma_tx_buf)
            .into_async(),
            framebuffer: Box::new_in(
                [0; LCD_WIDTH as usize * LCD_HEIGHT as usize],
                esp_alloc::ExternalMemory,
            ),
        }
    }

    pub async fn send_cmd(&mut self, cmd: u32, data: &[u8]) {
        self.spi
            .half_duplex_write_async(
                DataMode::Single,
                Command::_8Bit(LCD_OPCODE_WRITE_CMD, DataMode::Single),
                Address::_24Bit(cmd << 8, DataMode::Single),
                0,
                data,
            )
            .await
            .unwrap();
    }

    pub async fn send_color(&mut self, cmd: u32, data: &[u8]) {
        self.spi
            .half_duplex_write_async(
                DataMode::Quad,
                Command::_8Bit(LCD_OPCODE_WRITE_COLOR, DataMode::Single),
                Address::_24Bit(cmd << 8, DataMode::Single),
                0,
                data,
            )
            .await
            .unwrap();
    }

    pub async fn init(&mut self) {
        for entry in LCD_INIT_TABLE {
            self.send_cmd(entry.cmd, entry.data).await;

            if entry.delay_ms > 0 {
                Timer::after(Duration::from_millis(entry.delay_ms as u64)).await;
            }
        }
    }

    async fn set_address_window(&mut self, sx: u16, sy: u16, ex: u16, ey: u16) {
        self.send_cmd(
            0x2a,
            &[sx.to_be_bytes().to_vec(), ex.to_be_bytes().to_vec()].concat(),
        )
        .await;
        self.send_cmd(
            0x2b,
            &[sy.to_be_bytes().to_vec(), ey.to_be_bytes().to_vec()].concat(),
        )
        .await;
    }

    pub async fn flush(&mut self) {
        self.set_address_window(0, 0, LCD_WIDTH - 1, LCD_HEIGHT - 1)
            .await;

        const LINE_TO_SEND: usize = 48;

        for y in (0..480).step_by(LINE_TO_SEND) {
            let start = y * LCD_WIDTH as usize;
            let end = (y + LINE_TO_SEND) * LCD_WIDTH as usize;

            let data: &[u8] = unsafe {
                core::slice::from_raw_parts(
                    self.framebuffer[start..end].as_ptr() as *const u8,
                    self.framebuffer[start..end].len() * 2,
                )
            };

            if y == 0 {
                self.send_color(0x2c, &data).await;
            } else {
                self.send_color(0x3c, &data).await;
            }
        }
    }
}

impl OriginDimensions for LcdDisplay<'_> {
    fn size(&self) -> Size {
        Size::new(LCD_WIDTH as u32, LCD_HEIGHT as u32)
    }
}

impl DrawTarget for LcdDisplay<'_> {
    type Color = Rgb565;
    // `ExampleDisplay` uses a framebuffer and doesn't need to communicate with the display
    // controller to draw pixel, which means that drawing operations can never fail. To reflect
    // this the type `Infallible` was chosen as the `Error` type.
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            // Check if the pixel coordinates are out of bounds (negative or greater than
            // (63,63)). `DrawTarget` implementation are required to discard any out of bounds
            // pixels without returning an error or causing a panic.
            if let Ok((x @ 0..320, y @ 0..480)) = coord.try_into() {
                // Calculate the index in the framebuffer.
                let index = x + y * (LCD_WIDTH as u32);
                self.framebuffer[index as usize] = color.into_storage();
            }
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let color_value = color.into_storage();
        self.framebuffer.fill(color_value);

        Ok(())
    }
}
