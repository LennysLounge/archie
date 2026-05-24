#![allow(unused)]

use std::{
    fs,
    io::{self, Stdout, Write},
    panic,
    path::{Path, PathBuf},
    thread::{self, sleep},
    time::{Duration, Instant},
};

use archie::{
    isa::{self, Address::*, Instruction::*, Register::*},
    mcu::MCU,
};
use clap::Parser;
use crossterm::{
    ExecutableCommand, QueueableCommand,
    cursor::{DisableBlinking, Hide, MoveTo},
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, poll, read},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode},
};
use ratatui::{
    DefaultTerminal, Frame,
    prelude::*,
    symbols::border,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Row, Table, TableState, Widget},
};
use ux::u4;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(help = "The file to be assembled")]
    input_file: PathBuf,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let program = read_file_as_u16(&cli.input_file)?;
    let mut app = App {
        mcu: MCU::new(program),
        frames: 0,
        exit: false,
    };
    app.mcu.set_dbg_flag();

    ratatui::run(|terminal| app.run(terminal))?;
    Ok(())
}

struct App {
    mcu: MCU,
    frames: i32,
    exit: bool,
}

impl App {
    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut last_update = Instant::now();
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
                    break;
                }
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
            Constraint::Min(6),
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
    fn render_(&self, area: Rect, buf: &mut Buffer) {}
}
impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let v_layout = Layout::vertical([
            Constraint::Length(15),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .split(area);

        let h_layout = Layout::horizontal(vec![Constraint::Min(51), Constraint::Length(23)])
            .split(v_layout[0]);

        self.render_registers(h_layout[0], buf);
        self.render_cpu_status(h_layout[1], buf);

        let h_layout =
            Layout::horizontal(vec![Constraint::Fill(1), Constraint::Fill(1)]).split(v_layout[1]);

        self.render_disassembly(h_layout[0], buf);

        let block = Block::bordered()
            .borders(Borders::TOP)
            .border_style(Style::default().dark_gray());
        let inner = block.inner(v_layout[2]);
        block.render(v_layout[2], buf);

        let mut controls = vec![Line::from("Quit: <Q>")];
        if self.mcu.is_dbg_flag_set() {
            controls.push(Line::from("Single Step: <O>"));
            controls.push(Line::from("Run: <R>"));
        } else {
            Line::from("Stop: <B>");
        }

        let controls_layout =
            Layout::horizontal(controls.iter().map(|l| Constraint::Min(l.width() as u16)))
                .split(inner);
        for (control, area) in controls.iter().zip(controls_layout.iter()) {
            control.render(*area, buf);
        }
    }
}

fn app(terminal: &mut DefaultTerminal) -> io::Result<()> {
    loop {
        terminal.draw(|frame| {
            frame.render_widget("hello World", frame.area());
        });
        if read()?.is_key_press() {
            break;
        }
    }

    Ok(())
}

//     let cli = Cli::parse();

//     let program = read_file_as_u16(&cli.input_file)?;

//     enable_raw_mode()?;
//     get_next_key()?;

//     let mut stdout = io::stdout();
//     execute!(stdout, EnterAlternateScreen, DisableBlinking, Hide)?;

//     // Restore terminal on panic
//     let default_hook = panic::take_hook();
//     panic::set_hook(Box::new(move |info| {
//         let mut stdout = io::stdout();
//         stdout.execute(LeaveAlternateScreen).unwrap();
//         default_hook(info);
//     }));

//     let mut mcu = MCU::new(program);
//     mcu.set_dbg_flag();

//     let mut running = true;
//     while running {
//         //queue!(stdout, Clear(ClearType::All))?;
//         mcu.print_status(&mut stdout);

//         let (width, height) = crossterm::terminal::size()?;
//         queue!(stdout, MoveTo(0, height - 1), Print("Quit: Q    "),)?;
//         if mcu.is_dbg_flag_set() {
//             queue!(stdout, Print("Single step: O    Run: R    "))?;
//         } else {
//             queue!(stdout, Print("Break: B    "))?;
//         }
//         queue!(stdout, MoveTo(0, 14))?;

//         stdout.flush();
//         let key = get_next_key()?;
//         if let Some(key) = key {
//             match key {
//                 KeyEvent {
//                     code: KeyCode::Char('q'),
//                     ..
//                 }
//                 | KeyEvent {
//                     code: KeyCode::Char('c'),
//                     modifiers: KeyModifiers::CONTROL,
//                     ..
//                 } => {
//                     running = false;
//                 }
//                 KeyEvent {
//                     code: KeyCode::Char('o'),
//                     kind: crossterm::event::KeyEventKind::Press,
//                     ..
//                 } => {
//                     if mcu.is_dbg_flag_set() {
//                         mcu.run_one_cycle();
//                     }
//                 }
//                 KeyEvent {
//                     code: KeyCode::Char('r'),
//                     kind: crossterm::event::KeyEventKind::Press,
//                     ..
//                 } => {
//                     mcu.unset_dbg_flag();
//                 }
//                 KeyEvent {
//                     code: KeyCode::Char('b'),
//                     kind: crossterm::event::KeyEventKind::Press,
//                     ..
//                 } => {
//                     mcu.set_dbg_flag();
//                 }
//                 _ => (),
//             }
//         }
//         if !mcu.is_dbg_flag_set() && !mcu.is_halted() {
//             let start = Instant::now();
//             loop {
//                 for _ in 0..1000 {
//                     mcu.run_one_cycle();
//                     if mcu.is_dbg_flag_set() {
//                         break;
//                     }
//                 }
//                 let now = Instant::now();
//                 if now.duration_since(start) > Duration::from_millis(10)
//                     || mcu.is_dbg_flag_set()
//                     || mcu.is_halted()
//                 {
//                     break;
//                 }
//             }
//         } else {
//             thread::sleep(Duration::from_millis(10));
//         }
//     }
//     let (width, height) = crossterm::terminal::size()?;
//     execute!(stdout, LeaveAlternateScreen)?;
//     Ok(())
// }

fn get_next_key() -> io::Result<Option<KeyEvent>> {
    while poll(Duration::from_millis(10))? {
        match read()? {
            Event::Key(event) => return Ok(Some(event)),
            _ => (),
        }
    }
    Ok(None)
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
