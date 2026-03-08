use esp_hal::{interrupt::software::SoftwareInterrupt, system::Stack};
use esp_rtos::embassy::Executor;
use static_cell::StaticCell;

pub fn spawn_on_second_core<F>(
    cpu_control: esp_hal::peripherals::CPU_CTRL,
    int0: SoftwareInterrupt<'static, 0>,
    int1: SoftwareInterrupt<'static, 1>,
    f: F,
) where
    F: FnOnce(embassy_executor::Spawner) + Send + 'static,
{
    static APP_CORE_STACK: StaticCell<Stack<8192>> = StaticCell::new();
    let app_core_stack = APP_CORE_STACK.init(Stack::new());

    esp_rtos::start_second_core(cpu_control, int0, int1, app_core_stack, move || {
        static EXECUTOR: StaticCell<Executor> = StaticCell::new();
        let executor = EXECUTOR.init(Executor::new());
        executor.run(|spawner| {
            f(spawner);
        });
    });
}
