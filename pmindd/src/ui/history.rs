use std::{collections::HashMap, fmt::Debug, net::Ipv6Addr};

use ratatui::{
    prelude::{
        Alignment, Buffer, Color, Constraint, Direction, Layout, Margin, Rect, Style, Stylize,
    },
    symbols::Marker,
    text::Span,
    widgets::{
        block::Title, Axis, Block, Chart, Clear, Dataset, GraphType, LegendPosition, Padding, Row,
        Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Table, TableState, Widget,
    },
};

use crate::minder::Node;

#[derive(Clone, Debug)]
pub enum HistoryView {
    Lux,
    Lumens,
    Temp,
    Moisture,
}

struct Graph<'a> {
    points: Vec<(f64, f64)>,
    min_x: f64,
    label: String,
    y_labels: Vec<Span<'a>>,
    y_bounds: [f64; 2],
}

#[derive(Clone)]
pub struct NodeHistoryState {
    pub now: tokio::time::Instant,
    pub nodes: Vec<Node>,
}

impl Default for NodeHistoryState {
    fn default() -> Self {
        Self {
            now: tokio::time::Instant::now(),
            nodes: vec![],
        }
    }
}

impl NodeHistoryState {
    pub fn new(nodes: HashMap<Ipv6Addr, Node>) -> Self {
        Self {
            nodes: nodes.values().cloned().collect::<Vec<_>>(),
            ..Default::default()
        }
    }
}

#[derive(Clone)]
pub struct NodeHistory<'a> {
    active_row: usize,
    pub state: &'a NodeHistoryState,
    view: HistoryView,
}

impl<'a> NodeHistory<'a> {
    pub fn new(state: &'a NodeHistoryState, active_row: usize, view: HistoryView) -> Self {
        Self {
            active_row,
            state,
            view,
        }
    }

    fn render_nodes(&self, area: Rect, buf: &mut Buffer) {
        let rows: Vec<Row> = self
            .state
            .nodes
            .iter()
            .map(|entry| {
                let mut tmp: [u8; 8] = [0u8; 8];
                tmp[2..].copy_from_slice(&entry.eui);
                let id = format!("{:#X}", u64::from_be_bytes(tmp));
                let height: u16 = 1;
                Row::new(vec![id]).height(height)
            })
            .collect::<Vec<_>>();

        let max_rows = rows.len();
        let row = if max_rows != 0 {
            self.active_row % max_rows
        } else {
            0
        };

        let scrollbar_area = Rect {
            y: area.y + 2,
            height: area.height - 3,
            ..area
        };

        let mut s_state = ScrollbarState::default()
            .content_length(2)
            .viewport_content_length(1)
            .position(row);

        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol("▐")
            .render(scrollbar_area, buf, &mut s_state);

        let area = area.inner(Margin {
            vertical: 0,
            horizontal: 1,
        });

        let mut state = TableState::default().with_selected(Some(row));

        StatefulWidget::render(
            Table::new(rows, &[Constraint::Length(20)])
                .block(Block::new().style(Style::new().bg(Color::Black)))
                .header(Row::new(vec!["Node EUI"]).style(Style::new().bg(Color::Blue)))
                .highlight_style(Style::new().fg(Color::White).bg(Color::Blue)),
            area,
            buf,
            &mut state,
        );
    }

    fn render_lux(&self, area: Rect, buf: &mut Buffer, points: Vec<(f64, f64)>, min_x: f64) {
        let label = "Lux".to_string();
        let y_bounds = [0.0, 1100.0];
        let y_labels = vec!["0".bold(), "500".into(), "1100.0".bold()];

        self.render_graph(
            area,
            buf,
            Graph {
                points,
                min_x,
                label,
                y_labels,
                y_bounds,
            },
        )
    }

    fn render_lumens(
        &self,
        area: Rect,
        buf: &mut Buffer,
        points: Vec<(f64, f64)>,
        min_x: f64,
        max_l: f64,
    ) {
        let label = "Lumens (Full Spectrum)".to_string();
        let y_bounds = [max_l - 200.0, max_l + 100.0];
        let y_labels = vec![
            (max_l - 500.0).to_string().bold(),
            (max_l + 500.0).to_string().bold(),
        ];
        self.render_graph(
            area,
            buf,
            Graph {
                points,
                min_x,
                label,
                y_labels,
                y_bounds,
            },
        )
    }

    fn render_temp(&self, area: Rect, buf: &mut Buffer, points: Vec<(f64, f64)>, min_x: f64) {
        let label = "Temperature (F)".to_string();
        let y_bounds = [35.0, 105.0];
        let y_labels = vec!["35.0".bold(), "75.0".into(), "105.0".bold()];
        self.render_graph(
            area,
            buf,
            Graph {
                points,
                min_x,
                label,
                y_labels,
                y_bounds,
            },
        )
    }

    fn render_moisture(&self, area: Rect, buf: &mut Buffer, points: Vec<(f64, f64)>, min_x: f64) {
        let label = "Moisture".to_string();

        // NOTE this is tailored to seesaw sensor, will need to change at some point
        // for other sensor types
        let y_bounds = [200.0, 1100.0];
        let y_labels = vec!["Dry".bold(), "Wet".bold()];

        self.render_graph(
            area,
            buf,
            Graph {
                points,
                min_x,
                label,
                y_labels,
                y_bounds,
            },
        )
    }

    fn render_graph(&self, area: Rect, buf: &mut Buffer, graph: Graph) {
        let datasets = vec![Dataset::default()
            .marker(Marker::Braille)
            .style(Style::default().fg(Color::Yellow))
            .graph_type(GraphType::Line)
            .data(&graph.points)];

        Chart::new(datasets)
            .block(
                Block::bordered().title(
                    Title::default()
                        .content(graph.label.clone().cyan().bold())
                        .alignment(Alignment::Center),
                ),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .style(Style::default().gray())
                    .bounds([graph.min_x, graph.min_x + 1000.0]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::default().gray())
                    .bounds(graph.y_bounds)
                    .labels(graph.y_labels),
            )
            .legend_position(Some(LegendPosition::TopLeft))
            .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)))
            .render(area, buf);
    }
}

impl Widget for NodeHistory<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        crate::ui::Background.render(area, buf);
        let area = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });
        Clear.render(area, buf);

        Block::new()
            .title(format!("Node History: {:?}", self.view).bold().white())
            .title_alignment(Alignment::Center)
            .style(Style::new().bg(Color::Black))
            .padding(Padding::new(1, 1, 2, 1))
            .render(area, buf);

        let area = area.inner(Margin {
            vertical: 2,
            horizontal: 1,
        });

        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)])
            .split(area);

        self.render_nodes(area[0], buf);

        let num_nodes = self.state.nodes.len();
        let row = if num_nodes == 0 {
            0
        } else {
            self.active_row % num_nodes
        };

        let mut min_t: f64 = 0.0;
        let mut max_l: f64 = 0.0;

        if self.state.nodes.is_empty() || self.state.nodes[row].history.is_empty() {
            return;
        }

        let points = self.state.nodes[row]
            .history
            .iter()
            .map(|h| {
                if min_t == 0.0 {
                    min_t = h.data.timestamp as f64;
                }

                if (h.data.timestamp as f64) < min_t {
                    min_t = h.data.timestamp as f64;
                }

                if max_l == 0.0 {
                    max_l = h.data.light.unwrap_or_default().full_spectrum as f64;
                }

                if (h.data.light.unwrap_or_default().full_spectrum as f64) < max_l {
                    max_l = h.data.light.unwrap_or_default().full_spectrum as f64;
                }

                let timestamp = h.data.timestamp as f64;

                let reading = match self.view {
                    HistoryView::Lux => h.data.light.unwrap_or_default().lux as f64,
                    HistoryView::Lumens => h.data.light.unwrap_or_default().full_spectrum as f64,
                    HistoryView::Temp => h.data.soil.temperature as f64,
                    HistoryView::Moisture => h.data.soil.moisture as f64,
                };
                (
                    timestamp, // x is time
                    reading,   // y is the sensor reading
                )
            })
            .collect::<Vec<_>>();

        match self.view {
            HistoryView::Lux => self.render_lux(area[1], buf, points, min_t),
            HistoryView::Lumens => self.render_lumens(area[1], buf, points, min_t, max_l),
            HistoryView::Temp => self.render_temp(area[1], buf, points, min_t),
            HistoryView::Moisture => self.render_moisture(area[1], buf, points, min_t),
        }
    }
}
