use crossterm::{
	event::{
		self, DisableMouseCapture, EnableMouseCapture, Event,
		KeyCode::{self, Char},
		KeyEvent,
	},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::{Oid, Repository};
use std::{
	error::Error,
	io::{self, Stdout},
	path::Path,
};
use tui::{
	backend::{Backend, CrosstermBackend},
	layout::{Alignment, Constraint, Direction, Layout},
	style::{Color, Modifier, Style},
	text::{Span, Text},
	widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
	Frame, Terminal,
};

use crate::git;

pub struct App<'a> {
	pub blame: Vec<git::BlameLine>,
	blame_state: ListState,
	repo: &'a Repository,
	filepath: &'a Path,
	commit_stack: Vec<Oid>,
	line_history: Option<Text<'static>>, // output of git -L
}

impl App<'_> {
	pub fn new<'a>(repo: &'a Repository, filepath: &'a Path, commit: Oid) -> App<'a> {
		App {
			blame: vec![],
			blame_state: ListState::default(),
			repo,
			filepath,
			commit_stack: vec![commit],
			line_history: None,
		}
	}
}

type CrosstermTerm = Terminal<CrosstermBackend<Stdout>>;

pub fn setup() -> Result<CrosstermTerm, Box<dyn Error>> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
	let backend = CrosstermBackend::new(stdout);
	Ok(Terminal::new(backend)?)
}

pub fn run_app(terminal: &mut CrosstermTerm, mut app: App) -> Result<(), Box<dyn Error>> {
	loop {
		terminal.draw(|frame| ui(frame, &mut app))?;
		if let Event::Key(key) = event::read()? {
			match handle_input(&key, &mut app) {
				Ok(false) => {
					return Ok(());
				}
				Ok(true) => {} // ignored
				Err(err) => {
					terminal.draw(|frame| {
						frame.render_widget(
							Paragraph::new(format!("{}", err)).wrap(Wrap { trim: false }),
							tui::layout::Rect::new(0, 0, frame.size().width, 1),
						);
					})?;
					while !std::matches!(event::read()?, Event::Key(_)) {} // wait until any input to clear error
				}
			}
		}
	}
}

// returns whether to continue running the app
fn handle_input(key: &KeyEvent, app: &mut App) -> Result<bool, Box<dyn Error>> {
	match key {
		KeyEvent {
			code: Char('j') | KeyCode::Down,
			..
		} => match app.blame_state.selected() {
			Some(index) => {
				if index < app.blame.len() - 1 {
					app.blame_state.select(Some(index + 1));
				}
			}
			None => {
				app.blame_state.select(Some(0));
			}
		},
		KeyEvent {
			code: Char('k') | KeyCode::Up,
			..
		} => match app.blame_state.selected() {
			Some(index) => {
				if index > 0 {
					app.blame_state.select(Some(index - 1));
				}
			}
			None => {
				app.blame_state.select(Some(0));
			}
		},
		KeyEvent {
			code: KeyCode::Enter, ..
		} => {
			if let Some(index) = app.blame_state.selected() {
				let commit = app.commit_stack.last().unwrap();
				app.line_history = Some(git::log_follow(app.repo, app.filepath, index, *commit));
			}
		}
		KeyEvent { code: Char('b'), .. } => {
			if let Some(index) = app.blame_state.selected() {
				let parent = app.repo.find_commit(app.blame[index].commit)?.parent_id(0)?;
				app.blame = git::blame(&app.repo, app.filepath, parent)?;
				app.blame_state.select(Some(index.min(app.blame.len())));
				app.commit_stack.push(parent);
			}
		}
		KeyEvent { code: Char('B'), .. } => {
			if app.commit_stack.len() > 1 {
				app.commit_stack.pop();
				let commit = app.commit_stack.last().unwrap();
				app.blame = git::blame(&app.repo, app.filepath, *commit)?;
				if let Some(index) = app.blame_state.selected() {
					app.blame_state.select(Some(index.min(app.blame.len())));
				}
			}
		}
		KeyEvent {
			code: Char('q') | KeyCode::Esc,
			..
		} => {
			if app.line_history.is_some() {
				app.line_history = None
			} else {
				return Ok(false);
			}
		}
		_ => {} // ignored
	};
	return Ok(true);
}

pub fn teardown(terminal: &mut CrosstermTerm) {
	_ = disable_raw_mode();
	_ = execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
	_ = terminal.show_cursor();
}

fn ui<B: Backend>(frame: &mut Frame<B>, app: &mut App) {
	let constraints: &[Constraint];
	if app.line_history.is_none() {
		constraints = [Constraint::Percentage(100)].as_ref();
	} else {
		constraints = [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref();
	}
	let chunks = Layout::default()
		.direction(Direction::Horizontal)
		.constraints(constraints)
		.split(frame.size());

	let items: Vec<ListItem> = app.blame.iter().map(|line| ListItem::new(line.spans.clone())).collect();
	let title = Span::styled(
		app.commit_stack.last().unwrap().to_string(),
		Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
	);
	let list = List::new(items)
		.block(Block::default().title(title))
		.highlight_style(Style::default().bg(Color::DarkGray));
	frame.render_stateful_widget(list, chunks[0], &mut app.blame_state);

	if let Some(log) = &app.line_history {
		let paragraph = Paragraph::new(log.clone())
			.block(Block::default().borders(Borders::LEFT))
			.alignment(Alignment::Left);
		frame.render_widget(paragraph, chunks[1]);
	}
}
