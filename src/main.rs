use crossterm::event::Event;
use env_logger;
use log::{debug, info, warn};
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, Nvml};
use ratatui::{
    crossterm::event::{self, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::Style,
    symbols::Marker,
    widgets::{block::Title, Axis, Block, Chart, Dataset, Widget},
    DefaultTerminal, Frame,
};
use std::thread;
use std::time::Duration;

mod tests;

fn main() {
    env_logger::init();
    info!("Starting application");
    run_tui();
}

fn run_tui() {
    let mut terminal = ratatui::init();
    terminal.clear().expect("Failed to clear terminal");

    let mut app = NvidiaApp::default();
    let _app_result = app.run_app(&mut terminal);

    ratatui::restore();
}

#[derive(Debug, Default)]
struct NvidiaApp {
    gpus: Vec<GPUInfo>,
    exit: bool,
}

#[derive(Debug, Default)]
struct GPUInfo {
    core_clock: [u32; 30],
    temperature: [u32; 30],
    device_name: String,
}

impl NvidiaApp {
    pub fn run_app(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let nvml = Nvml::init().expect("Failed to initialize NVML");
        let device_count = nvml.device_count().expect("Failed to get device count");
        debug!("Found {} devices", device_count);

        if device_count == 0 {
            eprintln!("Error: No GPUs found. Please ensure that your system has NVIDIA GPUs installed and try again.");
            std::process::exit(1);
        }

        for i in 0..device_count {
            let gpu_device = nvml
                .device_by_index(i)
                .expect("Failed to get device by index");
            let device_name = gpu_device.name().expect("Failed to get GPU name");
            debug!("Found device: {}", device_name);
            self.gpus.push(GPUInfo {
                core_clock: [0; 30],
                temperature: [0; 30],
                device_name,
            });
        }

        while !self.exit {
            self.update_state()?;
            let _ = terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
            thread::sleep(Duration::from_secs(1));
        }
        Ok(())
    }

    fn update_state(&mut self) -> std::io::Result<()> {
        let nvml = Nvml::init().expect("Failed to initialize NVML");

        for (i, gpu_info) in self.gpus.iter_mut().enumerate() {
            let gpu_device = nvml
                .device_by_index(i.try_into().unwrap())
                .expect("Failed to get device by index");

            let current_clock = gpu_device
                .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
                .expect("Failed to retrieve GPU clock speed");
            debug!("GPU {} clock: {}", i, current_clock);
            gpu_info.core_clock.rotate_left(1);
            gpu_info.core_clock[29] = current_clock;

            let gpu_current_temperature = gpu_device
                .temperature(TemperatureSensor::Gpu)
                .expect("Failed to retrieve GPU temperature");
            debug!("GPU {} temperature: {}", i, gpu_current_temperature);
            gpu_info.temperature.rotate_left(1);
            gpu_info.temperature[29] = gpu_current_temperature;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        // if number of gpus is less than one then error and exit
        let num_gpus = self.gpus.len();
        if num_gpus == 0 {
            eprintln!("Error: No GPUs found. Please ensure that your system has NVIDIA GPUs installed and try again.");
            std::process::exit(1);
        }

        let percentage = 100 / num_gpus as u16;

        let constraints = vec![Constraint::Percentage(percentage); num_gpus];

        let chunks: Vec<Rect> = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(constraints)
            .split(frame.area())
            .to_vec();

        for (i, gpu_info) in self.gpus.iter().enumerate() {
            debug!("Drawing GPU {}: {}", i, gpu_info.device_name);
            let gpu_chunks: Vec<Rect> = Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[i])
                .to_vec();

            let clock_chunk = gpu_chunks[0];
            let temp_chunk = gpu_chunks[1];

            let clock_title = Title::from(format!("NVIDIA GPU Clock - {}", gpu_info.device_name));
            let clock_block = Block::bordered()
                .border_style(Style::new().fg(ratatui::style::Color::Rgb(117, 255, 0)))
                .title(clock_title.alignment(ratatui::layout::Alignment::Center));

            let temp_title =
                Title::from(format!("NVIDIA GPU Temperature - {}", gpu_info.device_name));
            let temp_block = Block::bordered()
                .border_style(Style::new().fg(ratatui::style::Color::Rgb(255, 0, 255)))
                .title(temp_title.alignment(ratatui::layout::Alignment::Center));

            let gpu_clock_data: Vec<(f64, f64)> = gpu_info
                .core_clock
                .iter()
                .zip(-29..=0)
                .map(|(clock, time)| (time as f64, *clock as f64))
                .collect();
            let gpu_temperature_data: Vec<(f64, f64)> = gpu_info
                .temperature
                .iter()
                .zip(-29..=0)
                .map(|(temp, time)| (time as f64, *temp as f64))
                .collect();

            let current_clock = gpu_info.core_clock[29].max(1); // Ensure the value is at least 1
            let current_temp = gpu_info.temperature[29].max(1); // Ensure the value is at least 1

            debug!("Current clock: {}", current_clock);
            debug!("Current temperature: {}", current_temp);

            let current_clock_str = current_clock.to_string();
            let current_temp_str = current_temp.to_string();

            let chart_gpu_clock_data = Dataset::default()
                .name("GPU Clock")
                .marker(Marker::Dot)
                .graph_type(ratatui::widgets::GraphType::Line)
                .data(&gpu_clock_data);
            let chart_gpu_temperature_data = Dataset::default()
                .name("GPU Temperature")
                .marker(Marker::Dot)
                .graph_type(ratatui::widgets::GraphType::Line)
                .data(&gpu_temperature_data);

            let chart_gpu_clock_x_axis = Axis::default()
                .title("Time")
                .bounds([-30.0, 0.0])
                .labels(vec!["Time"]);
            let chart_gpu_clock_y_axis = Axis::default()
                .title("GPU Clock Speed")
                .bounds([0.0, current_clock as f64])
                .labels(vec!["0", current_clock_str.as_str()]);

            let chart_gpu_temperature_x_axis = Axis::default()
                .title("Time")
                .bounds([-30.0, 0.0])
                .labels(vec!["Time"]);
            // For the temperature chart
            let chart_gpu_temperature_y_axis = Axis::default()
                .title("GPU Temperature")
                .bounds([0.0, current_temp as f64])
                .labels(vec!["0", current_temp_str.as_str()]);

            let chart_gpu_clock = Chart::new(vec![chart_gpu_clock_data])
                .block(clock_block.clone())
                .x_axis(chart_gpu_clock_x_axis)
                .y_axis(chart_gpu_clock_y_axis)
                .style(Style::new().fg(ratatui::style::Color::Rgb(48, 226, 173)));
            chart_gpu_clock.render(clock_chunk, frame.buffer_mut());

            let chart_gpu_temperature = Chart::new(vec![chart_gpu_temperature_data])
                .block(temp_block.clone())
                .x_axis(chart_gpu_temperature_x_axis)
                .y_axis(chart_gpu_temperature_y_axis)
                .style(Style::new().fg(ratatui::style::Color::Rgb(255, 0, 255)));
            chart_gpu_temperature.render(temp_chunk, frame.buffer_mut());
        }
    }

    fn handle_events(&mut self) -> std::io::Result<()> {
        if !event::poll(std::time::Duration::from_millis(150)).unwrap() {
            return Ok(()); // Don't try to read any events if there aren't any available
        }
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: event::KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.exit();
            }
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &NvidiaApp {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(
                self.gpus
                    .iter()
                    .map(|_| Constraint::Percentage(100 / self.gpus.len() as u16))
                    .collect::<Vec<Constraint>>(),
            )
            .split(area)
            .to_vec();

        for (i, gpu_info) in self.gpus.iter().enumerate() {
            let gpu_chunks: Vec<Rect> = Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[i])
                .to_vec();

            let clock_chunk = gpu_chunks[0];
            let temp_chunk = gpu_chunks[1];

            let clock_title = Title::from(format!("NVIDIA GPU Clock - {}", gpu_info.device_name));
            let clock_block = Block::bordered()
                .border_style(Style::new().fg(ratatui::style::Color::Rgb(117, 255, 0)))
                .title(clock_title.alignment(ratatui::layout::Alignment::Center));

            let temp_title =
                Title::from(format!("NVIDIA GPU Temperature - {}", gpu_info.device_name));
            let temp_block = Block::bordered()
                .border_style(Style::new().fg(ratatui::style::Color::Rgb(255, 0, 255)))
                .title(temp_title.alignment(ratatui::layout::Alignment::Center));

            let gpu_clock_data: Vec<(f64, f64)> = gpu_info
                .core_clock
                .iter()
                .zip(-29..=0)
                .map(|(clock, time)| (time as f64, *clock as f64))
                .collect();
            let gpu_temperature_data: Vec<(f64, f64)> = gpu_info
                .temperature
                .iter()
                .zip(-29..=0)
                .map(|(temp, time)| (time as f64, *temp as f64))
                .collect();

            let current_clock = gpu_info.core_clock[29].max(1); // Ensure the value is at least 1
            let current_temp = gpu_info.temperature[29].max(1); // Ensure the value is at least 1

            let current_clock_str = current_clock.to_string();
            let current_temp_str = current_temp.to_string();

            let chart_gpu_clock_data = Dataset::default()
                .name("GPU Clock")
                .marker(Marker::Dot)
                .graph_type(ratatui::widgets::GraphType::Line)
                .data(&gpu_clock_data);
            let chart_gpu_temperature_data = Dataset::default()
                .name("GPU Temperature")
                .marker(Marker::Dot)
                .graph_type(ratatui::widgets::GraphType::Line)
                .data(&gpu_temperature_data);

            let chart_gpu_clock_x_axis = Axis::default()
                .title("Time")
                .bounds([-30.0, 0.0])
                .labels(vec!["Time"]);
            let chart_gpu_clock_y_axis = Axis::default()
                .title("GPU Clock Speed")
                .bounds([0.0, current_clock as f64])
                .labels(vec![current_clock_str.as_str()]);

            let chart_gpu_temperature_x_axis = Axis::default()
                .title("Time")
                .bounds([-30.0, 0.0])
                .labels(vec!["Time"]);
            let chart_gpu_temperature_y_axis = Axis::default()
                .title("GPU Temperature")
                .bounds([0.0, current_temp as f64])
                .labels(vec![current_temp_str.as_str()]);

            let chart_gpu_clock = Chart::new(vec![chart_gpu_clock_data])
                .block(clock_block.clone())
                .x_axis(chart_gpu_clock_x_axis)
                .y_axis(chart_gpu_clock_y_axis)
                .style(Style::new().fg(ratatui::style::Color::Rgb(48, 226, 173)));
            chart_gpu_clock.render(clock_chunk, buf);

            let chart_gpu_temperature = Chart::new(vec![chart_gpu_temperature_data])
                .block(temp_block.clone())
                .x_axis(chart_gpu_temperature_x_axis)
                .y_axis(chart_gpu_temperature_y_axis)
                .style(Style::new().fg(ratatui::style::Color::Rgb(255, 0, 255)));
            chart_gpu_temperature.render(temp_chunk, buf);
        }
    }
}
