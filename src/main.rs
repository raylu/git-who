use std::fmt::Write;
use std::io::{stdin, stdout};
use std::process::exit;
use unsegen::base::{Color, StyleModifier, Terminal};
use unsegen::container::{Container, ContainerManager, ContainerProvider, HSplit, Leaf};
use unsegen::input::{Event, Input, Key, NavigateBehavior, ScrollBehavior};
use unsegen::widget::builtin::LogViewer;
use unsegen::widget::{RenderingHints, Widget};

struct Pager {
	buffer: LogViewer,
}

impl Pager {
	fn new() -> Self {
		Pager {
			buffer: LogViewer::new(),
		}
	}
}

impl Container<()> for Pager {
	fn input(&mut self, input: Input, _: &mut ()) -> Option<Input> {
		input
			.chain(
				ScrollBehavior::new(&mut self.buffer)
					.backwards_on(Key::Char('k'))
					.forwards_on(Key::Char('j')),
			)
			.finish()
	}
	fn as_widget<'a>(&'a self) -> Box<dyn Widget + 'a> {
		Box::new(self.buffer.as_widget())
	}
}

#[derive(Clone, PartialEq, Debug)]
enum Index {
	Left,
	Right,
}

struct App {
	left: Pager,
	right: Option<Pager>,
}

impl App {
	fn one_pane<'a>() -> Box<HSplit<'a, App>> {
		Box::new(HSplit::new(vec![(Box::new(Leaf::new(Index::Left)), 1.0)]))
	}
	fn two_pane<'a>() -> Box<HSplit<'a, App>> {
		Box::new(HSplit::new(vec![
			(Box::new(Leaf::new(Index::Left)), 0.5),
			(Box::new(Leaf::new(Index::Right)), 0.5),
		]))
	}
}

impl ContainerProvider for App {
	type Context = ();
	type Index = Index;
	fn get<'a, 'b: 'a>(&'b self, index: &'a Self::Index) -> &'b dyn Container<Self::Context> {
		match index {
			Index::Left => &self.left,
			Index::Right => match &self.right {
				Some(r) => r,
				None => &self.left,
			},
		}
	}
	fn get_mut<'a, 'b: 'a>(&'b mut self, index: &'a Self::Index) -> &'b mut dyn Container<Self::Context> {
		match index {
			Index::Left => &mut self.left,
			Index::Right => match &mut self.right {
				Some(ref mut r) => r,
				None => &mut self.left,
			},
		}
	}
	const DEFAULT_CONTAINER: Self::Index = Index::Left;
}

fn main() {
	let stdout = stdout();
	let stdin = stdin();
	let stdin = stdin.lock();

	let mut app = App {
		left: Pager::new(),
		right: None,
	};
	for _ in 1..10 {
		writeln!(app.left.buffer, "hi").unwrap();
	}
	let mut manager = ContainerManager::<App>::from_layout(App::one_pane());
	let mut term = Terminal::new(stdout.lock()).unwrap();

	manager.draw(
		term.create_root_window(),
		&mut app,
		StyleModifier::new().fg_color(Color::Yellow),
		RenderingHints::default(),
	);
	term.present();

	for input in Input::read_all(stdin) {
		let input = input
			.unwrap()
			.chain(manager.active_container_behavior(&mut app, &mut ()))
			.chain(
				NavigateBehavior::new(&mut manager.navigatable(&mut app))
					.left_on(Key::Char('h'))
					.right_on(Key::Char('l')),
			)
			.finish();
		if let Some(i) = input {
			if let Event::Key(key) = i.event {
				match key {
					Key::Char('\n') => {
						let mut right = Pager::new();
						for _ in 1..10 {
							writeln!(right.buffer, "bye").unwrap();
						}
						app.right = Some(right);
						manager.set_layout(App::two_pane());
						manager.set_active(Index::Right);
					}
					Key::Esc => {
						manager.set_layout(App::one_pane());
					}
					Key::Char('q') => {
						drop(term);
						exit(0);
					}
					_ => {} // ignored
				}
			}
		}
		manager.draw(
			term.create_root_window(),
			&mut app,
			StyleModifier::new().fg_color(Color::Yellow),
			RenderingHints::default(),
		);
		term.present();
	}
}
