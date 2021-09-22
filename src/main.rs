use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tui::backend::CrosstermBackend;
use tui::Terminal;

const DATA_PATH: &str = "./data/bingo_data.json"; // Todo: dig into const topic
const NUMBER_OF_CELLS: u16 = 25;
const BINGO_TEXT: &str = "\n\n
████    █████   █   █    ███     ███ 
█   █     █     ██  █   █   █   █   █
█   █     █     ███ █   █       █   █
████      █     █ █ █   █       █   █
█   █     █     █ █ █   █ ███   █   █
█   █     █     █  ██   █   █   █   █
█   █     █     █  ██   █   █   █   █
████    █████   █   █    ███     ███ 


Press 'q' to exit
";

enum Event<I> {
    Input(I),
    Tick,
}

#[derive(Clone, Copy)]
struct Cursor {
    position: Coordinate,
}

impl Cursor {
    fn default() -> Cursor {
        Cursor {
            position: Coordinate::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct BingoCell {
    string: String,
}

#[derive(Clone, Copy, Debug)]
struct Coordinate {
    x: u32,
    y: u32,
}

impl Coordinate {
    fn default() -> Coordinate {
        Coordinate { x: 0, y: 0 }
    }
}

// No used at this moment
#[derive(Copy, Clone, Debug)]
enum CurrentWindow {
    // Menu,
    Game,
    YouWon,
}

struct GameWindow {
    _header: tui::layout::Rect,
    main: tui::layout::Rect,
    footer: tui::layout::Rect,
}

impl GameWindow {
    fn new(
        _header: tui::layout::Rect,
        main: tui::layout::Rect,
        footer: tui::layout::Rect,
    ) -> GameWindow {
        GameWindow {
            _header,
            main,
            footer,
        }
    }
}

enum Action {
    Quit,
    ChangeScreen(CurrentWindow),
    Tick,
    Nop,
}

fn main() {
    // Initialization of data
    let all_bingo_cells = load_bingo_data();
    let game_cells = pick_bingo_set(all_bingo_cells);

    // Initialization of crossterm eventing
    let (tx, rx) = mpsc::channel();
    start_eventing_thread(tx);

    // Initialization of terminal
    enable_raw_mode().expect("Something went wrong during enabling raw mode"); // Todo: needs proper error handling
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut cursor = Cursor::default();
    let mut all_shots = Vec::new();

    let mut current_screen = CurrentWindow::Game;

    terminal.clear().unwrap();

    loop {
        // drawing
        terminal
            .draw(|rect| {
                let size = rect.size();
                let chunks = tui::layout::Layout::default() // TODO move this outside to not create this every loop
                    .direction(tui::layout::Direction::Vertical)
                    .margin(2)
                    .constraints(
                        [
                            tui::layout::Constraint::Length(3),
                            tui::layout::Constraint::Min(2),
                            tui::layout::Constraint::Length(3),
                        ]
                        .as_ref(),
                    )
                    .split(size); // todo: move this part to seperate function

                let game_window = GameWindow::new(chunks[0], chunks[1], chunks[2]);

                draw_header(&game_window, rect);
                draw_grid(
                    &game_cells,
                    &game_window,
                    current_screen,
                    rect,
                    &all_shots,
                    cursor,
                );

                draw_footer(&game_window, rect);
            })
            .unwrap(); // drawing end

        // key handling
        match handle_input(&rx, &mut all_shots, &mut cursor) {
            Action::Quit => {
                disable_raw_mode().unwrap();
                terminal.clear().unwrap();
                terminal.show_cursor().unwrap();
                break;
            }
            Action::ChangeScreen(new_screen) => {
                current_screen = new_screen;
            }
            _ => continue,
        }
    }
}

fn draw_grid(
    game_data: &[BingoCell],
    game_window: &GameWindow,
    current_screen: CurrentWindow,
    frame: &mut tui::Frame<tui::backend::CrosstermBackend<std::io::Stdout>>,
    bingo_choices: &[Coordinate],
    cursor: Cursor,
) {
    match current_screen {
        CurrentWindow::Game => {
            let number_of_elements: usize = 5;
            let default_constraints: [tui::layout::Constraint; 5] =
                [tui::layout::Constraint::Percentage(100 / number_of_elements as u16); 5];
            let main_block = tui::layout::Layout::default()
                .direction(tui::layout::Direction::Vertical)
                .constraints(default_constraints.as_ref())
                .split(game_window.main);
            let mut box_counter = 1;
            for (i, &main_block_item) in main_block.iter().enumerate().take(number_of_elements) {
                let row = tui::layout::Layout::default()
                    .direction(tui::layout::Direction::Horizontal)
                    .constraints(default_constraints.as_ref())
                    .split(main_block_item);
                for (j, &cell) in row.iter().enumerate().take(number_of_elements) {
                    let mut style = tui::style::Style::default().fg(tui::style::Color::White);
                    if i == cursor.position.x as usize && j == cursor.position.y as usize {
                        style = tui::style::Style::default()
                            .fg(tui::style::Color::Black)
                            .bg(tui::style::Color::White);
                    }
                    if bingo_choices
                        .iter()
                        .any(|cord| cord.x == j as u32 && cord.y == i as u32)
                    {
                        style = tui::style::Style::default()
                            .fg(tui::style::Color::Black)
                            .bg(tui::style::Color::Cyan);

                        if i == cursor.position.x as usize && j == cursor.position.y as usize {
                            style = tui::style::Style::default()
                                .fg(tui::style::Color::Black)
                                .bg(tui::style::Color::LightCyan);
                        }
                    }
                    let bingo_box = tui::widgets::Block::default()
                        .borders(tui::widgets::Borders::ALL)
                        .style(style)
                        .title(format!("Box {}", box_counter))
                        .border_type(tui::widgets::BorderType::Plain);
                    let text = vec![tui::text::Spans::from(vec![tui::text::Span::raw(
                        game_data[box_counter - 1].string.clone(),
                    )])];
                    let bingo_paragrapgh = tui::widgets::Paragraph::new(text)
                        .block(bingo_box)
                        .alignment(tui::layout::Alignment::Center)
                        .wrap(tui::widgets::Wrap { trim: true });
                    frame.render_widget(bingo_paragrapgh, cell);
                    box_counter += 1;
                }
            }
        }
        CurrentWindow::YouWon => {
            let you_won_text = tui::widgets::Paragraph::new(BINGO_TEXT)
                .style(tui::style::Style::default().fg(tui::style::Color::White))
                .alignment(tui::layout::Alignment::Center)
                .block(
                    tui::widgets::Block::default()
                        .borders(tui::widgets::Borders::ALL)
                        .title("Win")
                        .border_type(tui::widgets::BorderType::Plain),
                );

            frame.render_widget(you_won_text, game_window.main);
        }
        // CurrentWindow::Menu => {}
    }
}

fn draw_footer(
    game_window: &GameWindow,
    frame: &mut tui::Frame<tui::backend::CrosstermBackend<std::io::Stdout>>,
) {
    let copyright = tui::widgets::Paragraph::new("tui-bingo 2021 - all rights reserved")
        .style(tui::style::Style::default().fg(tui::style::Color::LightCyan))
        .alignment(tui::layout::Alignment::Center)
        .block(
            tui::widgets::Block::default()
                .borders(tui::widgets::Borders::ALL)
                .title("Copyright")
                .border_type(tui::widgets::BorderType::Plain),
        );

    frame.render_widget(copyright, game_window.footer);
}

fn draw_header(
    _game_window: &GameWindow,
    _frame: &mut tui::Frame<tui::backend::CrosstermBackend<std::io::Stdout>>,
) {
}

fn handle_input(
    rx: &mpsc::Receiver<Event<crossterm::event::KeyEvent>>,
    all_shots: &mut Vec<Coordinate>,
    cursor: &mut Cursor,
) -> Action {
    match rx.recv().unwrap() {
        Event::Input(event) => match event.code {
            crossterm::event::KeyCode::Char('q') => Action::Quit,
            crossterm::event::KeyCode::Down => {
                if cursor.position.x < 4 {
                    cursor.position.x += 1;
                }
                Action::Nop
            }
            crossterm::event::KeyCode::Up => {
                if cursor.position.x > 0 {
                    cursor.position.x -= 1;
                }
                Action::Nop
            }
            crossterm::event::KeyCode::Left => {
                if cursor.position.y > 0 {
                    cursor.position.y -= 1;
                }
                Action::Nop
            }
            crossterm::event::KeyCode::Right => {
                if cursor.position.y < 4 {
                    cursor.position.y += 1;
                }
                Action::Nop
            }
            crossterm::event::KeyCode::Enter => mark_shot(
                all_shots,
                Coordinate {
                    x: cursor.position.y,
                    y: cursor.position.x,
                },
            ),
            _ => Action::Nop,
        },
        Event::Tick => Action::Tick,
    }
}

fn start_eventing_thread(tx: mpsc::Sender<Event<crossterm::event::KeyEvent>>) {
    let tick_rate = Duration::from_millis(200);

    thread::spawn(move || {
        let mut last_tick = Instant::now();

        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout).expect("Polling fucked up!") {
                if let crossterm::event::Event::Key(key) =
                    crossterm::event::read().expect("Reading event fucked up")
                {
                    tx.send(Event::Input(key))
                        .expect("Everything going to fuck up");
                }
            }

            if last_tick.elapsed() >= tick_rate && tx.send(Event::Tick).is_ok() {
                last_tick = Instant::now();
            }
        }
    });
}

fn load_bingo_data() -> Vec<BingoCell> {
    let file_content = fs::read_to_string(DATA_PATH).expect("TODO! Proper error handling");
    let bingo_data: Vec<BingoCell> =
        serde_json::from_str(&file_content).expect("TODO! Proper error hangling");
    bingo_data
}

fn pick_bingo_set(mut all_data: Vec<BingoCell>) -> Vec<BingoCell> {
    let mut game_set: Vec<BingoCell> = Vec::new();
    let mut rng = rand::thread_rng();

    for i in 0..NUMBER_OF_CELLS {
        let index = rng.gen_range(0..NUMBER_OF_CELLS - i);
        game_set.push(all_data.remove(index as usize));
    }
    game_set
}

fn mark_shot(all_shots: &mut Vec<Coordinate>, shot: Coordinate) -> Action {
    // TODO: improve that
    if let Some(index) = all_shots
        .iter()
        .position(|cord| cord.x == shot.x && cord.y == shot.y)
    {
        all_shots.remove(index);
    } else {
        all_shots.push(shot);
    }

    let mut shots_count = HashMap::new();

    for shot in all_shots {
        let count = shots_count.entry(format!("x{}", shot.x)).or_insert(0);
        *count += 1;
        let count = shots_count.entry(format!("y{}", shot.y)).or_insert(0);
        *count += 1;
    }
    for (_cord, count) in shots_count {
        if count >= 5 {
            return Action::ChangeScreen(CurrentWindow::YouWon);
        }
    }
    Action::Nop
}
