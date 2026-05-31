use std::{
    fs,
    io::{self},
    path::{Path, PathBuf},
    thread::sleep,
    time::{Duration, Instant},
};

use archie::mcu::MCU;
use clap::Parser;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, poll, read};
use ratatui::{
    DefaultTerminal, Frame,
    prelude::*,
    style::Styled,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Row, Table, Widget},
};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tui_logger::{TuiLoggerSmartWidget, TuiWidgetEvent, TuiWidgetState};
use tui_term::{vt100, widget::PseudoTerminal};

#[derive(Parser, Debug)]
struct Cli {
    #[arg(help = "The file to be assembled")]
    input_file: PathBuf,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    tui_logger::init_logger(tui_logger::LevelFilter::Trace)?;
    tui_logger::set_default_level(tui_logger::LevelFilter::Trace);

    tracing_subscriber::registry()
        .with(tui_logger::TuiTracingSubscriberLayer)
        .init();

    let cli = Cli::parse();
    let program = read_file_as_u16(&cli.input_file)?;
    let mut app = App {
        mcu: MCU::new(program),
        frames: 0,
        exit: false,
        logger_state: TuiWidgetState::default(),
        current_screen: Screen::McuState,
        terminal: vt100::Parser::new(24, 80, 30),
    };
    app.mcu.set_dbg_flag();

    ratatui::run(|terminal| app.run(terminal))?;
    Ok(())
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Screen {
    McuState,
    Logging,
    Terminal,
}
impl Screen {
    const SCREENS: [Screen; 3] = [Screen::McuState, Screen::Logging, Screen::Terminal];
    fn name(&self) -> &str {
        match self {
            Screen::McuState => "MCU State",
            Screen::Logging => "Logging",
            Screen::Terminal => "Serial Terminal",
        }
    }
}

struct App {
    mcu: MCU,
    frames: i32,
    exit: bool,
    logger_state: TuiWidgetState,
    current_screen: Screen,
    terminal: vt100::Parser,
}

impl App {
    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut last_update = Instant::now();
        info!("hello world, starting main loop now");
        warn!("Warning!!");
        error!("!error!");

        self.terminal.screen_mut().set_scrollback(30);
        info!("scrollback: {}", self.terminal.screen().scrollback());

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;

            loop {
                if !self.mcu.is_dbg_flag_set() && !self.mcu.is_halted() {
                    for _ in 0..1000 {
                        self.mcu.run_one_cycle();
                        if self.mcu.is_dbg_flag_set() || self.mcu.is_halted() {
                            break;
                        }
                    }
                } else {
                    sleep(Duration::from_millis(16));
                }
                let now = Instant::now();
                if now.duration_since(last_update).as_millis() > 16 {
                    last_update = now;
                    break;
                }
            }
            if !self.mcu.serial_out.is_empty() {
                self.terminal.process(&self.mcu.serial_out);
                self.mcu.serial_out.clear();
            }
            self.frames += 1;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        while poll(Duration::ZERO)? {
            match read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => (),
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Char(c @ '0'..='9') => {
                let num = c.to_digit(10).unwrap_or(0);
                let index = match num {
                    0 => 10,
                    _ => num - 1,
                };
                self.current_screen = *Screen::SCREENS
                    .get(index as usize)
                    .unwrap_or(&Screen::McuState);
            }
            _ => match self.current_screen {
                Screen::McuState => match key_event.code {
                    KeyCode::Char('o') => {
                        if self.mcu.is_dbg_flag_set() {
                            self.mcu.run_one_cycle();
                        }
                    }
                    KeyCode::Char('r') => {
                        self.mcu.unset_dbg_flag();
                    }
                    KeyCode::Char('b') => {
                        self.mcu.set_dbg_flag();
                    }
                    _ => (),
                },
                Screen::Logging => {
                    let evt = match key_event.code {
                        KeyCode::Char('h') => Some(TuiWidgetEvent::HideKey),
                        KeyCode::Char('f') => Some(TuiWidgetEvent::FocusKey),
                        KeyCode::Up => Some(TuiWidgetEvent::UpKey),
                        KeyCode::Down => Some(TuiWidgetEvent::DownKey),
                        KeyCode::Left => Some(TuiWidgetEvent::LeftKey),
                        KeyCode::Right => Some(TuiWidgetEvent::RightKey),
                        KeyCode::Char('-') => Some(TuiWidgetEvent::MinusKey),
                        KeyCode::Char('+') => Some(TuiWidgetEvent::PlusKey),
                        KeyCode::PageUp => Some(TuiWidgetEvent::PrevPageKey),
                        KeyCode::PageDown => Some(TuiWidgetEvent::NextPageKey),
                        KeyCode::Esc => Some(TuiWidgetEvent::EscapeKey),
                        KeyCode::Char(' ') => Some(TuiWidgetEvent::SpaceKey),
                        _ => None,
                    };
                    if let Some(e) = evt {
                        self.logger_state.transition(e);
                    }
                }
                Screen::Terminal => (),
            },
        }
    }

    fn render_registers(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().dark_gray());
        let inner_area = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(2),
                Constraint::Min(8),
                Constraint::Length(2),
            ])
            .split(inner_area);

        Paragraph::new(
            format!("General purpose registers, frames: {}", self.frames)
                .white()
                .bold(),
        )
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .padding(Padding::horizontal(2))
                .border_style(Style::default().dark_gray()),
        )
        .render(layout[0], buf);

        let mut rows = Vec::new();
        for i in 0..8 {
            rows.push(Row::new(vec![
                Line::from(format!("R{}", i)).left_aligned().light_blue(),
                Line::from(format!("{:04X}", self.mcu.register[i]))
                    .right_aligned()
                    .light_cyan(),
                Line::from(format!("{}", self.mcu.register[i]))
                    .right_aligned()
                    .light_cyan(),
                Line::from(format!("{}", self.mcu.register[i] as i16))
                    .right_aligned()
                    .light_cyan(),
                "│".dark_gray().into(),
                Line::from(format!("R{}", i + 8))
                    .left_aligned()
                    .light_blue(),
                Line::from(format!("{:04X}", self.mcu.register[i + 8]))
                    .right_aligned()
                    .light_cyan(),
                Line::from(format!("{}", self.mcu.register[i + 8]))
                    .right_aligned()
                    .light_cyan(),
                Line::from(format!("{}", self.mcu.register[i + 8] as i16))
                    .right_aligned()
                    .light_cyan(),
            ]));
        }
        let widths = [
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Min(5),
            Constraint::Min(6),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Min(5),
            Constraint::Min(6),
        ];
        Widget::render(
            Table::new(rows, widths),
            layout[1].inner(Margin::new(2, 0)),
            buf,
        );

        let footer_block = Block::default()
            .borders(Borders::TOP)
            .padding(Padding::horizontal(2))
            .border_style(Style::default().dark_gray());
        let footer_area = footer_block.inner(layout[2]);
        footer_block.render(layout[2], buf);

        let footer_layout = Layout::horizontal(vec![
            Constraint::Min(9),
            Constraint::Min(9),
            Constraint::Min(9),
        ])
        .split(footer_area);

        Line::from(vec![
            "ST".light_blue(),
            format!(" {:#06X}", self.mcu.register[13]).dark_gray(),
        ])
        .render(footer_layout[0], buf);

        Line::from(vec![
            "SP".light_blue(),
            format!(" {:#06X}", self.mcu.register[14]).dark_gray(),
        ])
        .render(footer_layout[1], buf);
        Line::from(vec![
            "PC".light_blue(),
            format!(" {:#06X}", self.mcu.register[15]).dark_gray(),
        ])
        .render(footer_layout[2], buf);
    }

    fn render_cpu_status(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().dark_gray());
        let inner_area = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(2),
                Constraint::Min(8),
                Constraint::Length(2),
            ])
            .split(inner_area);

        Paragraph::new("CPU Status".white().bold())
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .padding(Padding::horizontal(2))
                    .border_style(Style::default().dark_gray()),
            )
            .render(layout[0], buf);
        let flags = [
            ("ZF", "Zero", self.mcu.is_zero_flag_set()),
            ("NF", "Negative", self.mcu.is_negative_flag_set()),
            ("CF", "Carry", self.mcu.is_carry_flag_set()),
            ("OF", "Overflow", self.mcu.is_overflow_flag_set()),
            (
                "IF",
                "IQR enable",
                self.mcu.is_interrupt_request_enabled_flag_set(),
            ),
            ("DF", "Debug", self.mcu.is_dbg_flag_set()),
            ("HF", "Halt", self.mcu.is_halted()),
        ];
        let mut rows = Vec::new();
        for (mnemonic, desc, b) in flags {
            let color = if b { Color::Green } else { Color::DarkGray };
            rows.push(Row::new(vec![
                mnemonic.light_blue(),
                desc.fg(color),
                format!("[{}]", b as u8).fg(color),
            ]));
        }
        let widths = vec![
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(3),
        ];
        Widget::render(
            Table::new(rows, widths).column_highlight_style(Style::new().red().italic()),
            layout[1].inner(Margin::new(2, 0)),
            buf,
        );

        Paragraph::new(vec![
            "Cycles:".into(),
            Line::from(self.mcu.cycle_counter.to_string())
                .right_aligned()
                .light_cyan(),
        ])
        .render(layout[2].inner(Margin::new(2, 0)), buf);
    }

    fn render_disassembly(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().dark_gray());
        let inner_area = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(2), Constraint::Fill(1)])
            .split(inner_area);

        Paragraph::new("Program".white().bold())
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .padding(Padding::horizontal(2))
                    .border_style(Style::default().dark_gray()),
            )
            .render(layout[0], buf);

        let min = (self.mcu.register[15]).saturating_sub(layout[1].height & !1);
        let widths = vec![
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Min(6),
        ];
        let rows = (0..=layout[1].height)
            .into_iter()
            .map(|row_num| {
                let addr = min.wrapping_add(row_num * 2) as usize;
                let value = self.mcu.rom.get(addr / 2);
                let is_pc = if addr == self.mcu.register[15] as usize {
                    "PC->".to_owned()
                } else {
                    "".to_string()
                };
                let addr_span = format!("{:#06X}", addr).dark_gray();
                if let Some(value) = value {
                    Row::new(vec![
                        is_pc.into(),
                        addr_span,
                        format!("{:04X}", value).light_cyan(),
                    ])
                } else {
                    Row::new(vec![is_pc.into(), addr_span, "".into()])
                }
            })
            .collect::<Vec<_>>();
        Widget::render(
            Table::new(rows, widths),
            layout[1].inner(Margin::new(2, 0)),
            buf,
        );
    }
}
impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).split(area);

        match self.current_screen {
            Screen::McuState => {
                let v_layout = Layout::vertical([Constraint::Length(15), Constraint::Fill(1)])
                    .split(layout[0]);

                let h_layout =
                    Layout::horizontal(vec![Constraint::Min(51), Constraint::Length(23)])
                        .split(v_layout[0]);

                self.render_registers(h_layout[0], buf);
                self.render_cpu_status(h_layout[1], buf);

                let h_layout = Layout::horizontal(vec![Constraint::Fill(1), Constraint::Fill(1)])
                    .split(v_layout[1]);

                self.render_disassembly(h_layout[0], buf);
            }
            Screen::Logging => {
                TuiLoggerSmartWidget::default()
                    .state(&self.logger_state)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().dark_gray())
                    .style_error(Style::default().red())
                    .style_warn(Style::default().yellow())
                    .render(layout[0], buf);
            }
            Screen::Terminal => {
                let (rows, cols) = self.terminal.screen().size();
                PseudoTerminal::new(self.terminal.screen())
                    .block(
                        Block::bordered()
                            .border_type(BorderType::Rounded)
                            .padding(Padding::new(2, 2, 0, 0))
                            .title("Terminal".white().bold())
                            .border_style(Style::default().dark_gray()),
                    )
                    .render(
                        layout[0]
                            .centered(Constraint::Length(cols + 6), Constraint::Length(rows + 2)),
                        buf,
                    );
            }
        }

        let block = Block::bordered()
            .borders(Borders::TOP)
            .border_style(Style::default().dark_gray());
        let controls_row_area = block.inner(layout[1]);
        block.render(layout[1], buf);

        let mut controls = Vec::new();

        controls.push(
            Line::from(
                Screen::SCREENS
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        format!("[{}]", i + 1).set_style(if &self.current_screen == s {
                            Style::default().white().bold()
                        } else {
                            Style::default().dark_gray()
                        })
                    })
                    .chain([" ".into(), self.current_screen.name().into()])
                    .collect::<Vec<_>>(),
            )
            .centered(),
        );
        match self.current_screen {
            Screen::McuState => {
                if self.mcu.is_dbg_flag_set() {
                    controls.push(Line::from("Single Step: <O>").centered());
                    controls.push(Line::from("Run: <R>").centered());
                } else {
                    controls.push(Line::from("Stop: <B>").centered());
                }
            }
            Screen::Logging => {
                controls.push(Line::from("Hide: <H>").centered());
            }
            Screen::Terminal => (),
        }
        controls.push(Line::from("Quit: <Q>").centered());

        let controls_layout = Layout::horizontal(
            controls
                .iter()
                .map(|l| Constraint::Length(l.width() as u16)),
        )
        .flex(layout::Flex::SpaceBetween)
        .split(controls_row_area);
        for (control, area) in controls.iter().zip(controls_layout.iter()) {
            control.render(*area, buf);
        }
    }
}

fn read_file_as_u16(path: &impl AsRef<Path>) -> io::Result<Vec<u16>> {
    let bytes = fs::read(path)?;

    if bytes.len() % 2 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File length is not a multiple of 2",
        ));
    }

    let vec_u16 = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    Ok(vec_u16)
}
