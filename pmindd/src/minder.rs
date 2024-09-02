use pmind_broker::{Eui, NodeSensorReading, NodeState, NodeStatus, Registration};
#[cfg(feature = "database")]
use pmindb::PlantMinderDatabase;

use std::{collections::HashMap, net::Ipv6Addr};
use tokio::sync::mpsc::UnboundedReceiver;

use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen, SetSize};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Direction, Layout, Margin, Rect},
    prelude::{Constraint, CrosstermBackend, Span, Stylize},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{
        Cell, Clear, HighlightSpacing, Paragraph, Row, StatefulWidget, Table, TableState, Tabs,
        Widget,
    },
    Terminal, TerminalOptions, Viewport,
};
use std::{io, panic};

use crate::{event::AppCmd, ui::NodeHistoryState, PlantMinderError};

pub type PlantMinderResult<T> = std::result::Result<T, PlantMinderError>;

const TABS: &[&str] = &["Nodes", "Moisture", "Temp", "Lux", "Lumens"];
pub const NUM_TABS: usize = 5;
const CMD_KEYS: &[(&str, &str)] = &[
    ("q", ": exit //"),
    ("tab", ": next view //"),
    ("backtab", ": prev view //"),
    ("↑", ": scroll node up //"),
    ("↓", ": scroll node down"),
];

//#[derive(Debug)]
pub struct PlantMinder {
    pub running: bool,
    pub state: TableState,
    pub data_queue_rx: UnboundedReceiver<NodeSensorReading>,

    pub nodes: HashMap<Ipv6Addr, Node>,
    pub node_addrs: HashMap<Eui, Ipv6Addr>,
    read_timeout: u64,
    pub tab: usize,
    pub row: usize,
    pub window_start: usize,
    pub window_end: usize,
    #[cfg(feature = "database")]
    pub db: Option<Box<dyn PlantMinderDatabase>>,
}

pub const MAX_WINDOW: usize = 50;

#[derive(Clone, Debug)]
pub struct Node {
    pub addr: Ipv6Addr,
    pub eui: Eui,
    pub name: String,
    pub history: Vec<NodeSensorReading>,
    pub state: NodeState,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            addr: Ipv6Addr::from(0u128),
            eui: [0u8; 6],
            name: String::default(),
            history: Vec::with_capacity(MAX_WINDOW),
            state: NodeState::Unknown,
        }
    }
}

impl PlantMinder {
    pub fn new(read_timeout: u64, data_queue_rx: UnboundedReceiver<NodeSensorReading>) -> Self {
        Self {
            running: true,
            state: TableState::default().with_selected(0),
            data_queue_rx,
            node_addrs: HashMap::new(),
            nodes: HashMap::new(),
            read_timeout,
            tab: 0,
            row: 0,
            window_start: 0,
            window_end: 0,
            #[cfg(feature = "database")]
            db: None,
        }
    }

    pub async fn tick(&mut self) {
        let mut buffer: Vec<NodeSensorReading> = vec![];
        self.recv_many(&mut buffer, 20).await;
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    #[cfg(feature = "database")]
    pub fn enable_database(
        &mut self,
        db: impl PlantMinderDatabase + 'static,
    ) -> PlantMinderResult<()> {
        self.db = Some(std::boxed::Box::new(db));
        Ok(())
    }

    pub async fn recv_many(&mut self, buffer: &mut Vec<NodeSensorReading>, limit: usize) {
        let size = tokio::select! {
            size = self.data_queue_rx.recv_many(buffer, limit) => {
                size
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.read_timeout)) => {
                0
            }
        };

        if size != 0 {
            let keys = self.nodes.clone().into_keys().collect::<Vec<Ipv6Addr>>();

            keys.iter().for_each(|&key| {
                let new_data = buffer
                    .iter()
                    .filter_map(|&e| if *e.addr.ip() == key { Some(e) } else { None })
                    .collect::<Vec<NodeSensorReading>>();

                if !new_data.is_empty() {
                    self.new_data(key, new_data);
                }
            });
        }
    }

    pub async fn node_registration(&mut self, reg: Registration) {
        log::info!(
            "Received plant registrition for eui: {:?}, addr {:?}, plant name {:?}",
            reg.0,
            reg.1,
            reg.2
        );
        let history = {
            if let Some(previous) = self.node_addrs.get(&reg.0) {
                if let Some(entry) = self.nodes.get(previous) {
                    entry.history.clone()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        };

        // Evict the old addr from both hashmaps
        if !history.is_empty() {
            let previous = self.node_addrs.get(&reg.0);
            if let Some(p) = previous {
                self.nodes.remove(p);
            }
            self.node_addrs.remove(&reg.0);
        }

        self.node_addrs
            .entry(reg.0)
            .and_modify(|a| *a = reg.1)
            .or_insert(reg.1);

        self.nodes
            .entry(reg.1)
            .and_modify(|a| a.history.clone_from(&history))
            .or_insert(Node {
                history,
                addr: reg.1,
                eui: reg.0,
                name: reg.2,
                state: NodeState::Online,
            });
    }

    fn new_data(&mut self, key: Ipv6Addr, data: Vec<NodeSensorReading>) {
        let mut drained = false;
        self.nodes
            .entry(key)
            .and_modify(|a| {
                if a.history.len() + data.len() >= MAX_WINDOW {
                    // bump the 5 oldest readings
                    a.history.drain(..5);
                    drained = true;
                }
                a.history.extend_from_slice(&data)
            })
            .or_default();
    }

    fn render_node_last_table(&self, area: Rect, buf: &mut Buffer) {
        let header_style = Style::default().fg(Color::White).bg(Color::Cyan);

        let header = ["Plant Name", "Ipv6 Addr", "State"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = self.nodes.iter().map(|(addr, node)| {
            // apply this general rule of thumb for now,
            // it is highly tailored to the seesaw moisture sensor
            let node_state = {
                if node.history.is_empty() {
                    "Waiting".to_string()
                } else {
                    let last = node.history.len();
                    match node.history[last - 1].data.soil.moisture {
                        750..=1500 => "Good & moist".to_string(),
                        501..=749 => "Ok (for now)".to_string(),
                        401..=500 => "Danger Zone".to_string(),
                        250..=400 => "WATER ME!".to_string(),
                        _ => "Invalid reading".to_string(),
                    }
                }
            };

            Row::new(vec![node.name.clone(), addr.to_string(), node_state])
                .style(Style::new().fg(Color::Cyan).bg(Color::Black))
                .height(3)
        });

        let t = Table::new(
            rows,
            [
                Constraint::Min(20),
                Constraint::Length(50),
                Constraint::Min(10),
            ],
        )
        .header(header)
        .bg(Color::Black)
        .highlight_spacing(HighlightSpacing::Always);

        let mut state = TableState::default();
        StatefulWidget::render(t, area, buf, &mut state);
    }

    fn render_bottom(&self, area: Rect, buf: &mut Buffer) {
        let spans = CMD_KEYS
            .iter()
            .flat_map(|(key, desc)| {
                let key = Span::styled(
                    format!(" {} ", key),
                    Style::new().fg(Color::Cyan).bg(Color::Black),
                );
                let desc = Span::styled(
                    format!(" {} ", desc),
                    Style::new().fg(Color::Cyan).bg(Color::Black),
                );
                [key, desc]
            })
            .collect::<Vec<_>>();

        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .fg(Color::Cyan)
            .bg(Color::Black)
            .render(area, buf)
    }

    fn render_top(&self, area: Rect, buf: &mut Buffer) {
        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(85)])
            .split(area);

        Paragraph::new(Span::styled(
            "Plant Minder",
            Style::new()
                .bg(Color::Black)
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .render(area[0], buf);

        Tabs::new(TABS.to_vec())
            .style(
                Style::new()
                    .bg(Color::Black)
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_style(
                Style::new()
                    .bg(Color::Black)
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED),
            )
            .select(self.tab)
            .divider("")
            .render(area[1], buf);
    }

    fn render_selection(&self, area: Rect, buf: &mut Buffer) {
        let menu_index = self.tab;

        match menu_index {
            0 => {
                self.render_node_last_table(area, buf);
            }
            1 => {
                crate::ui::NodeHistory::new(
                    &NodeHistoryState::new(self.nodes.clone()),
                    self.row,
                    crate::ui::HistoryView::Moisture,
                )
                .render(area, buf);
            }
            2 => {
                crate::ui::NodeHistory::new(
                    &NodeHistoryState::new(self.nodes.clone()),
                    self.row,
                    crate::ui::HistoryView::Temp,
                )
                .render(area, buf);
            }
            3 => {
                crate::ui::NodeHistory::new(
                    &NodeHistoryState::new(self.nodes.clone()),
                    self.row,
                    crate::ui::HistoryView::Lux,
                )
                .render(area, buf);
            }
            4 => {
                crate::ui::NodeHistory::new(
                    &NodeHistoryState::new(self.nodes.clone()),
                    self.row,
                    crate::ui::HistoryView::Lumens,
                )
                .render(area, buf);
            }
            _ => {
                self.render_node_last_table(area, buf);
            }
        };
    }
}

pub async fn handle_app_cmd(cmd: AppCmd, app: &mut PlantMinder) {
    match cmd {
        AppCmd::Quit => app.quit(),
        AppCmd::Next => {
            if app.tab == NUM_TABS - 1 {
                app.tab = 0;
            } else {
                app.tab += 1;
            }
        }
        AppCmd::Back => {
            if app.tab == 0 {
                app.tab = NUM_TABS - 1;
            } else {
                app.tab -= 1;
            }
        }
        AppCmd::Down => {
            app.row += 1;
        }
        AppCmd::Up => {
            if app.row == 0 {
                if !app.node_addrs.is_empty() {
                    app.row = app.node_addrs.len() - 1;
                } else {
                    app.row = 0;
                }
            } else {
                app.row -= 1;
            }
        }
        e => {
            log::debug!("Unimplemented event received {e:?}");
            // drop it for now
        }
    }
}

pub async fn handle_node_state_change(event: NodeStatus, app: &mut PlantMinder) {
    match event {
        NodeStatus::Registration(reg) => app.node_registration(reg).await,
        NodeStatus::Termination((_addr, _error_state)) => {
            // TODO
            log::info!("TODO! report this in rendered display");
        }
    }
}

impl Widget for &mut PlantMinder {
    fn render(self, area: Rect, buf: &mut Buffer) {
        crate::ui::Background.render(area, buf);
        let area = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });
        Clear.render(area, buf);

        let area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

        self.render_top(area[0], buf);
        self.render_selection(area[1], buf);
        self.render_bottom(area[2], buf);
    }
}

#[derive(Debug)]
pub struct Tui {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
    col: u16,
    row: u16,
}

impl Tui {
    // TODO toml file, make this configurable
    // for now optimize for my LCD screen
    const TERM_COL: u16 = 125;
    const TERM_ROW: u16 = 35;

    pub fn new() -> Result<Self, std::io::Error> {
        // keep track of prev terminal size before resizing
        let (col, row) = crossterm::terminal::size()?;
        crossterm::execute!(std::io::stdout(), SetSize(Self::TERM_COL, Self::TERM_ROW),)?;

        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, Self::TERM_COL, Self::TERM_ROW)),
        };

        Ok(Self {
            terminal: Terminal::with_options(CrosstermBackend::new(io::stderr()), options)?,
            col,
            row,
        })
    }

    /// Using ratatui template for now
    pub fn init(&mut self) -> PlantMinderResult<()> {
        terminal::enable_raw_mode()?;
        crossterm::execute!(io::stderr(), EnterAlternateScreen)?;

        let panic_hook = panic::take_hook();
        let col = self.col;
        let row = self.row;
        panic::set_hook(Box::new(move |panic| {
            Self::reset(col, row).expect("failed to reset the terminal");
            panic_hook(panic);
        }));

        self.terminal.clear()?;
        Ok(())
    }

    pub fn draw(&mut self, app: &mut PlantMinder) -> PlantMinderResult<()> {
        self.terminal
            .draw(|frame| frame.render_widget(app, frame.size()))?;
        Ok(())
    }

    fn reset(col: u16, row: u16) -> PlantMinderResult<()> {
        terminal::disable_raw_mode()?;
        crossterm::execute!(io::stderr(), LeaveAlternateScreen)?;
        crossterm::execute!(io::stderr(), crossterm::terminal::SetSize(col, row))?;
        Ok(())
    }

    pub fn exit(&mut self) -> PlantMinderResult<()> {
        Self::reset(self.col, self.row)?;
        Ok(())
    }
}
