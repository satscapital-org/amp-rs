//! Asset Display TUI Example
//!
//! This example demonstrates a beautiful terminal UI for displaying comprehensive
//! asset information from the AMP API using ratatui.
//!
//! This demo showcases:
//! - Asset metadata (name, ticker, precision, registration status)
//! - Circulation statistics (issued, distributed, burned, etc.)
//! - Current asset holders with their addresses and balances
//! - Real-time data from the Blockstream AMP API
//!
//! Usage: cargo run --example asset_display_tui
//!
//! Make sure to set up your .env file with AMP_USERNAME and AMP_PASSWORD

use amp_rs::signer::LwkSoftwareSigner;
use amp_rs::ApiClient;
use amp_rs::ElementsRpc;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use tokio::runtime::Runtime;

// Demo asset information
const ASSET_UUID: &str = "bc2d31af-60d0-4346-bfba-11b045f92dff";
const DEMO_CATEGORY_NAME: &str = "SatsCapital Demo Category";

struct AssetDisplayData {
    name: String,
    ticker: String,
    asset_uuid: String,
    asset_id: String,
    precision: i64,
    domain: String,
    is_registered: bool,
    is_authorized: bool,
    is_locked: bool,
    transfer_restricted: bool,
    issuer_id: i64,
    // Summary data
    issued: i64,
    reissued: i64,
    assigned: i64,
    distributed: i64,
    burned: i64,
    blacklisted: i64,
    registered_users: i64,
    active_registered_users: i64,
    // Ownership data
    holders: Vec<(Option<String>, i64, Option<String>)>, // (owner user_id, amount, optional GAID)
}

#[derive(Clone, PartialEq)]
enum AppScreen {
    Main,
    DistributionInput,
}

#[derive(Clone)]
struct DistributionInput {
    gaid: String,
    amount: String,
    cursor_pos: usize, // 0 for GAID, 1 for amount
    error: Option<String>,
}

impl DistributionInput {
    fn new() -> Self {
        Self {
            gaid: String::new(),
            amount: String::new(),
            cursor_pos: 0,
            error: None,
        }
    }
}

#[derive(Clone)]
struct DistributionProgress {
    messages: Vec<String>,
    in_progress: bool,
    complete: bool,
}

enum DistributionMessage {
    Info(String),
    Success(String),
    Error(String),
    Complete,
}

impl DistributionProgress {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            in_progress: false,
            complete: false,
        }
    }

    fn add_message(&mut self, msg: String) {
        self.messages.push(msg);
    }

    fn add_info(&mut self, msg: &str) {
        self.add_message(format!("ℹ️  {}", msg));
    }

    fn add_success(&mut self, msg: &str) {
        self.add_message(format!("✓ {}", msg));
    }

    fn add_error(&mut self, msg: &str) {
        self.add_message(format!("✗ {}", msg));
    }
}

struct AppState {
    screen: AppScreen,
    asset_data: AssetDisplayData,
    distribution_input: DistributionInput,
    distribution_progress: DistributionProgress,
    distribution_rx: Option<Receiver<DistributionMessage>>,
    is_reloading: bool,
}

impl AssetDisplayData {
    fn calculate_circulation(&self) -> i64 {
        self.issued + self.reissued
    }

    fn calculate_available(&self) -> i64 {
        self.calculate_circulation() - self.distributed - self.burned - self.blacklisted
    }

    fn format_amount(&self, amount: i64) -> String {
        let divisor = 10_i64.pow(self.precision as u32);
        let whole = amount / divisor;
        let fractional = amount % divisor;
        format!(
            "{}.{:0width$}",
            whole,
            fractional,
            width = self.precision as usize
        )
    }
}

async fn fetch_asset_data() -> Result<AssetDisplayData, Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    let client = ApiClient::new().await?;

    // Fetch asset details
    let asset = client.get_asset(ASSET_UUID).await?;

    // Fetch asset summary for circulation data
    let summary = client.get_asset_summary(ASSET_UUID).await?;

    // Fetch ownership data
    let ownerships = client.get_asset_ownerships(ASSET_UUID, None).await?;

    // Convert ownership data to holders list and sort with issuer first
    let issuer_id_str = asset.issuer.to_string();
    let mut holders: Vec<(Option<String>, i64, Option<String>)> = ownerships
        .into_iter()
        .map(|o| (o.owner, o.amount, o.gaid))
        .collect();

    // Sort holders to put issuer first, then by balance descending
    holders.sort_by(|a, b| {
        let a_is_issuer = a.0.as_ref().map(|s| s.as_str()) == Some(issuer_id_str.as_str());
        let b_is_issuer = b.0.as_ref().map(|s| s.as_str()) == Some(issuer_id_str.as_str());

        match (a_is_issuer, b_is_issuer) {
            (true, false) => std::cmp::Ordering::Less, // Issuer comes first
            (false, true) => std::cmp::Ordering::Greater, // Issuer comes first
            _ => b.1.cmp(&a.1),                        // Otherwise sort by balance descending
        }
    });

    Ok(AssetDisplayData {
        name: asset.name,
        ticker: asset.ticker.unwrap_or_else(|| "N/A".to_string()),
        asset_uuid: asset.asset_uuid,
        asset_id: asset.asset_id,
        precision: asset.precision,
        domain: asset.domain.unwrap_or_else(|| "N/A".to_string()),
        is_registered: asset.is_registered,
        is_authorized: asset.is_authorized,
        is_locked: asset.is_locked,
        transfer_restricted: asset.transfer_restricted,
        issuer_id: asset.issuer,
        issued: summary.issued,
        reissued: summary.reissued,
        assigned: summary.assigned,
        distributed: summary.distributed,
        burned: summary.burned,
        blacklisted: summary.blacklisted,
        registered_users: summary.registered_users,
        active_registered_users: summary.active_registered_users,
        holders,
    })
}

fn ui(f: &mut Frame, app: &AppState) {
    let size = f.area();
    let data = &app.asset_data;

    // Create main layout with header, content, and footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(size);

    // Header
    let header_text = vec![Line::from(vec![
        Span::styled("📊 ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{} ({})", data.name, data.ticker),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" - "),
        Span::styled(
            "SatsCapital Asset Display",
            Style::default().fg(Color::Gray),
        ),
    ])];

    let header = Paragraph::new(header_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Asset Information ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    match app.screen {
        AppScreen::Main => {
            // Content area - split into left and right columns
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            // Left column - split into asset details and circulation stats
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(content_chunks[0]);

            // Asset Details Section
            render_asset_details(f, left_chunks[0], data);

            // Circulation Statistics Section
            render_circulation_stats(f, left_chunks[1], data);

            // Right column - Holders list
            render_holders_list(f, content_chunks[1], data);
        }
        AppScreen::DistributionInput => {
            render_distribution_screen(
                f,
                chunks[1],
                &app.distribution_input,
                &app.distribution_progress,
            );
        }
    }

    // Footer with instructions - vary based on screen and distribution state
    let footer_text = if app.screen == AppScreen::Main {
        vec![Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::Gray)),
            Span::styled(
                "'d'",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to distribute  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "'r'",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to reload  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "'q'",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" or ", Style::default().fg(Color::Gray)),
            Span::styled(
                "'Esc'",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to exit", Style::default().fg(Color::Gray)),
        ])]
    } else if app.distribution_progress.in_progress {
        // During distribution, show that keys are disabled
        vec![Line::from(vec![
            Span::styled(
                "Distribution in progress... ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Please wait", Style::default().fg(Color::Gray)),
        ])]
    } else {
        // On distribution input screen, no reload option
        vec![Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::Gray)),
            Span::styled(
                "'q'",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" or ", Style::default().fg(Color::Gray)),
            Span::styled(
                "'Esc'",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to exit", Style::default().fg(Color::Gray)),
        ])]
    };

    let footer = Paragraph::new(footer_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);

    // Render reload indicator overlay if reloading (on top of everything)
    if app.is_reloading {
        use ratatui::widgets::Clear;
        let overlay_area = centered_rect(30, 15, size);
        f.render_widget(Clear, overlay_area);

        let reload_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("⟳ ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    "Reloading...",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Fetching latest asset data",
                Style::default().fg(Color::Gray),
            )]),
        ];

        let reload_widget = Paragraph::new(reload_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Reload ")
                    .title_style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
            )
            .alignment(Alignment::Center);
        f.render_widget(reload_widget, overlay_area);
    }
}

fn render_asset_details(f: &mut Frame, area: Rect, data: &AssetDisplayData) {
    let details = vec![
        Line::from(vec![
            Span::styled(
                "Name: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &data.name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "UUID: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&data.asset_uuid),
        ]),
        Line::from(vec![
            Span::styled(
                "Asset ID: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&data.asset_id[..32]),
        ]),
        Line::from(vec![
            Span::raw("          "),
            Span::raw(&data.asset_id[32..]),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Ticker: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &data.ticker,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "Precision: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(data.precision.to_string()),
        ]),
        Line::from(vec![
            Span::styled(
                "Domain: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&data.domain),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Status: ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("  ● Registered: "),
            Span::styled(
                if data.is_registered {
                    "Yes ✓"
                } else {
                    "No ✗"
                },
                if data.is_registered {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  ● Authorized: "),
            Span::styled(
                if data.is_authorized {
                    "Yes ✓"
                } else {
                    "No ✗"
                },
                if data.is_authorized {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  ● Locked: "),
            Span::styled(
                if data.is_locked {
                    "Yes 🔒"
                } else {
                    "No 🔓"
                },
                if data.is_locked {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  ● Transfer Restricted: "),
            Span::styled(
                if data.transfer_restricted {
                    "Yes"
                } else {
                    "No"
                },
                if data.transfer_restricted {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
    ];

    let paragraph = Paragraph::new(details)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" Asset Details ")
                .title_style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

/// Helper function to create a centered rectangle for overlays
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_circulation_stats(f: &mut Frame, area: Rect, data: &AssetDisplayData) {
    let circulation = data.calculate_circulation();
    let available = data.calculate_available();

    let stats = vec![
        Line::from(vec![
            Span::styled(
                "Total Circulation: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                data.format_amount(circulation),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Issued: ", Style::default().fg(Color::White)),
            Span::raw(data.format_amount(data.issued)),
        ]),
        Line::from(vec![
            Span::styled("  Reissued: ", Style::default().fg(Color::White)),
            Span::raw(data.format_amount(data.reissued)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Distribution: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Assigned: ", Style::default().fg(Color::White)),
            Span::raw(data.format_amount(data.assigned)),
        ]),
        Line::from(vec![
            Span::styled("  Distributed: ", Style::default().fg(Color::White)),
            Span::styled(
                data.format_amount(data.distributed),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Available: ", Style::default().fg(Color::White)),
            Span::styled(
                data.format_amount(available),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Special: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Burned: ", Style::default().fg(Color::White)),
            Span::styled(
                data.format_amount(data.burned),
                if data.burned > 0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("  Blacklisted: ", Style::default().fg(Color::White)),
            Span::styled(
                data.format_amount(data.blacklisted),
                if data.blacklisted > 0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Users: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Registered: ", Style::default().fg(Color::White)),
            Span::raw(data.registered_users.to_string()),
        ]),
        Line::from(vec![
            Span::styled("  Active: ", Style::default().fg(Color::White)),
            Span::raw(data.active_registered_users.to_string()),
        ]),
    ];

    let paragraph = Paragraph::new(stats)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(" Circulation Statistics ")
                .title_style(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_holders_list(f: &mut Frame, area: Rect, data: &AssetDisplayData) {
    let total_holders = data.holders.len();
    let total_held: i64 = data.holders.iter().map(|(_, amount, _)| amount).sum();
    let total_circulation = data.calculate_circulation();

    // Split the area into header and scrollable content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Header section
            Constraint::Min(0),    // Holders list
        ])
        .split(area);

    // Render header summary
    let header_items = vec![
        ListItem::new(Line::from(vec![
            Span::styled(
                "Total Holders: ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                total_holders.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                "Total Circulation: ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                data.format_amount(total_held),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![Span::styled(
            "━".repeat(50),
            Style::default().fg(Color::Gray),
        )])),
    ];

    let header_list = List::new(header_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(format!(" Asset Holders ({}) ", total_holders))
            .title_style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    f.render_widget(header_list, chunks[0]);

    // Calculate how many lines each holder entry needs
    let lines_per_holder = if data.holders.iter().any(|(_, _, gaid)| gaid.is_some()) {
        7 // Title + gauge + address + GAID + balance + separator
    } else {
        6 // Title + gauge + address + balance + separator
    };

    // Render holders with individual blocks containing gauges
    let available_height = chunks[1].height.saturating_sub(2); // Account for outer borders
    let holders_to_show = (available_height as usize / lines_per_holder).min(data.holders.len());

    if data.holders.is_empty() {
        let empty_msg = Paragraph::new(Line::from(vec![Span::styled(
            "No holders found",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .alignment(Alignment::Center);
        f.render_widget(empty_msg, chunks[1]);
        return;
    }

    // Create constraints for each holder block
    let mut constraints = Vec::new();
    for _ in 0..holders_to_show {
        constraints.push(Constraint::Length(lines_per_holder as u16));
    }
    if holders_to_show < data.holders.len() {
        constraints.push(Constraint::Min(0)); // Filler space
    }

    let holder_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(chunks[1]);

    // Render each holder in its own block with gauge
    for (idx, (i, (owner, amount, gaid))) in data
        .holders
        .iter()
        .enumerate()
        .take(holders_to_show)
        .enumerate()
    {
        let percentage_of_supply = (*amount as f64 / total_circulation as f64) * 100.0;
        let owner_str = owner.as_ref().map(|s| s.as_str()).unwrap_or("Unknown");
        let is_issuer = Some(data.issuer_id.to_string()) == *owner;

        // Format owner display (truncate if too long)
        let owner_display = if owner_str.len() > 45 {
            format!(
                "{}...{}",
                &owner_str[..20],
                &owner_str[owner_str.len() - 20..]
            )
        } else {
            owner_str.to_string()
        };

        // Create holder info lines
        let mut holder_lines = vec![Line::from(vec![
            Span::styled(format!("#{} ", i + 1), Style::default().fg(Color::Gray)),
            if is_issuer {
                Span::styled(
                    "ISSUER ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("")
            },
            Span::styled(
                format!("{:.2}% of supply", percentage_of_supply),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];

        // Add user ID line
        holder_lines.push(Line::from(vec![
            Span::styled("User ID: ", Style::default().fg(Color::Cyan)),
            Span::raw(owner_display.clone()),
        ]));

        // Add GAID if present
        if let Some(gaid_value) = gaid {
            holder_lines.push(Line::from(vec![
                Span::styled("GAID: ", Style::default().fg(Color::Cyan)),
                Span::styled(gaid_value, Style::default().fg(Color::Green)),
            ]));
        }

        // Add balance line
        holder_lines.push(Line::from(vec![
            Span::styled("Balance: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                data.format_amount(*amount),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        let holder_info = Paragraph::new(holder_lines).block(Block::default());

        // Split holder chunk into info and gauge
        let holder_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(if gaid.is_some() { 4 } else { 3 }), // Info lines
                Constraint::Length(1),                                  // Gauge
                Constraint::Length(1),                                  // Spacing
            ])
            .split(holder_chunks[idx]);

        f.render_widget(holder_info, holder_layout[0]);

        // Render gauge showing percentage of total supply
        let gauge_ratio = (percentage_of_supply / 100.0).min(1.0);
        let gauge_color = if percentage_of_supply >= 50.0 {
            Color::Red
        } else if percentage_of_supply >= 25.0 {
            Color::Yellow
        } else if percentage_of_supply >= 10.0 {
            Color::Cyan
        } else {
            Color::Green
        };

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
            .ratio(gauge_ratio)
            .label(format!("{:.2}%", percentage_of_supply));

        f.render_widget(gauge, holder_layout[1]);
    }

    // Show indicator if there are more holders
    if holders_to_show < data.holders.len() {
        let remaining = data.holders.len() - holders_to_show;
        let more_msg = Paragraph::new(Line::from(vec![Span::styled(
            format!("... and {} more holder(s)", remaining),
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]))
        .alignment(Alignment::Center);
        if let Some(last_chunk) = holder_chunks.last() {
            f.render_widget(more_msg, *last_chunk);
        }
    }
}

fn render_distribution_screen(
    f: &mut Frame,
    area: Rect,
    form: &DistributionInput,
    progress: &DistributionProgress,
) {
    // Split left (form) and right (progress)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_distribution_input(f, chunks[0], form, progress);
    render_distribution_status(f, chunks[1], progress);
}

fn render_distribution_input(
    f: &mut Frame,
    area: Rect,
    form: &DistributionInput,
    progress: &DistributionProgress,
) {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Distribute Asset",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "GAID: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(if form.gaid.is_empty() {
                "<enter GAID>".to_string()
            } else {
                form.gaid.clone()
            }),
        ]),
        Line::from(vec![
            Span::styled(
                "Amount (BTC): ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(if form.amount.is_empty() {
                "<e.g. 0.00000001>".to_string()
            } else {
                form.amount.clone()
            }),
        ]),
        Line::from(""),
    ];

    if progress.in_progress {
        lines.push(Line::from(vec![
            Span::styled(
                "Status: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Distribution in progress...",
                Style::default().fg(Color::Yellow),
            ),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "See right panel for live updates",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]));
    } else if progress.complete {
        lines.push(Line::from(vec![
            Span::styled(
                "Status: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("✓ Complete!", Style::default().fg(Color::Green)),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "Press Esc to return to main screen",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]));
    } else {
        lines.push(Line::from(vec![Span::styled(
            "Instructions:",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Gray)),
            Span::raw("Type GAID and amount"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Gray)),
            Span::raw("Press "),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" to start distribution"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Gray)),
            Span::raw("Press "),
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" to switch fields"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Gray)),
            Span::raw("Press "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" to cancel"),
        ]));

        if let Some(err) = &form.error {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "✗ Error: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(err.clone(), Style::default().fg(Color::Red)),
            ]));
        }
    }

    // Add cursor indicator
    if !progress.in_progress && !progress.complete {
        lines.push(Line::from(""));
        let cursor_text = if form.cursor_pos == 0 {
            "GAID"
        } else {
            "Amount"
        };
        lines.push(Line::from(vec![
            Span::styled("▶ ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("Editing: {}", cursor_text),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" Distribution Form ")
                .title_style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn render_distribution_status(f: &mut Frame, area: Rect, progress: &DistributionProgress) {
    let title = if progress.complete {
        " ✓ Distribution Complete "
    } else if progress.in_progress {
        " ↻ Distribution Status "
    } else {
        " Status Log "
    };

    let border_color = if progress.complete {
        Color::Green
    } else if progress.in_progress {
        Color::Yellow
    } else {
        Color::Blue
    };

    // Take last messages that fit in the display area
    let available_lines = (area.height.saturating_sub(3)) as usize; // Account for borders and title
    let display_messages: Vec<Line> = progress
        .messages
        .iter()
        .rev()
        .take(available_lines)
        .rev()
        .map(|msg| {
            // Parse the message to determine color
            if msg.starts_with("✓") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Green)))
            } else if msg.starts_with("✗") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Red)))
            } else if msg.starts_with("ℹ️") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Cyan)))
            } else {
                Line::from(msg.clone())
            }
        })
        .collect();

    let paragraph = Paragraph::new(display_messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(title)
                .title_style(
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
    rt: &Runtime,
) -> io::Result<()> {
    loop {
        // Check for distribution progress updates
        let mut should_clear_rx = false;
        if let Some(rx) = &app.distribution_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    DistributionMessage::Info(s) => app.distribution_progress.add_info(&s),
                    DistributionMessage::Success(s) => app.distribution_progress.add_success(&s),
                    DistributionMessage::Error(s) => app.distribution_progress.add_error(&s),
                    DistributionMessage::Complete => {
                        app.distribution_progress.in_progress = false;
                        app.distribution_progress.complete = true;
                        should_clear_rx = true;
                    }
                }
            }
        }
        if should_clear_rx {
            app.distribution_rx = None;
        }

        terminal.draw(|f| ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Block all key inputs during distribution
                if app.distribution_progress.in_progress {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => {
                        // If on main screen, quit; otherwise return to main
                        if app.screen == AppScreen::Main {
                            return Ok(());
                        } else {
                            // Act like Esc - return to main and reload
                            app.screen = AppScreen::Main;

                            app.is_reloading = true;
                            terminal.draw(|f| ui(f, app))?;

                            match rt.block_on(async { fetch_asset_data().await }) {
                                Ok(new_data) => {
                                    app.asset_data = new_data;
                                }
                                Err(e) => {
                                    eprintln!("Failed to reload asset data: {}", e);
                                }
                            }

                            app.is_reloading = false;
                        }
                    }
                    KeyCode::Esc => {
                        // Esc returns to main screen if in a sub-screen
                        if app.screen != AppScreen::Main {
                            app.screen = AppScreen::Main;

                            // Auto-reload data when returning to main screen
                            app.is_reloading = true;
                            terminal.draw(|f| ui(f, app))?;

                            match rt.block_on(async { fetch_asset_data().await }) {
                                Ok(new_data) => {
                                    app.asset_data = new_data;
                                }
                                Err(e) => {
                                    eprintln!("Failed to reload asset data: {}", e);
                                }
                            }

                            app.is_reloading = false;
                        }
                    }
                    KeyCode::Char('d') if app.screen == AppScreen::Main => {
                        app.screen = AppScreen::DistributionInput;
                        app.distribution_input = DistributionInput::new();
                        app.distribution_progress = DistributionProgress::new();
                    }
                    KeyCode::Char('r') if app.screen == AppScreen::Main && !app.is_reloading => {
                        // Set reloading flag
                        app.is_reloading = true;

                        // Trigger a redraw to show the indicator
                        terminal.draw(|f| ui(f, app))?;

                        // Reload asset data in the background
                        match rt.block_on(async { fetch_asset_data().await }) {
                            Ok(new_data) => {
                                app.asset_data = new_data;
                            }
                            Err(e) => {
                                eprintln!("Failed to reload asset data: {}", e);
                            }
                        }

                        // Clear reloading flag
                        app.is_reloading = false;
                    }
                    KeyCode::Tab if app.screen == AppScreen::DistributionInput => {
                        app.distribution_input.cursor_pos =
                            (app.distribution_input.cursor_pos + 1) % 2;
                    }
                    KeyCode::Backspace if app.screen == AppScreen::DistributionInput => {
                        let field = app.distribution_input.cursor_pos;
                        if field == 0 {
                            app.distribution_input.gaid.pop();
                        } else {
                            app.distribution_input.amount.pop();
                        }
                    }
                    KeyCode::Enter
                        if app.screen == AppScreen::DistributionInput
                            && !app.distribution_progress.in_progress =>
                    {
                        // Validate inputs
                        let gaid = app.distribution_input.gaid.trim().to_string();
                        let amount_str = app.distribution_input.amount.trim().to_string();
                        if gaid.is_empty() || amount_str.is_empty() {
                            app.distribution_input.error =
                                Some("GAID and amount are required".to_string());
                        } else if amount_str
                            .parse::<f64>()
                            .ok()
                            .filter(|v| *v > 0.0)
                            .is_none()
                        {
                            app.distribution_input.error =
                                Some("Amount must be a positive number in BTC units".to_string());
                        } else {
                            app.distribution_input.error = None;
                            app.distribution_progress = DistributionProgress::new();
                            app.distribution_progress.in_progress = true;

                            // Create channel for progress updates
                            let (tx, rx) = mpsc::channel();
                            app.distribution_rx = Some(rx);

                            // Spawn distribution in background task
                            let gaid_clone = gaid.clone();
                            let amount_btc = amount_str.parse::<f64>().unwrap();
                            let asset_uuid = app.asset_data.asset_uuid.clone();

                            rt.spawn(async move {
                                run_distribution_flow_with_channel(
                                    &asset_uuid,
                                    gaid_clone,
                                    amount_btc,
                                    tx,
                                )
                                .await;
                            });
                        }
                    }
                    KeyCode::Char(c) if app.screen == AppScreen::DistributionInput => {
                        if !c.is_control() {
                            if app.distribution_input.cursor_pos == 0 {
                                app.distribution_input.gaid.push(c);
                            } else {
                                app.distribution_input.amount.push(c);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn run_distribution_flow_with_channel(
    asset_uuid: &str,
    gaid: String,
    amount_btc: f64,
    tx: Sender<DistributionMessage>,
) {
    // Load env
    dotenvy::dotenv().ok();

    let _ = tx.send(DistributionMessage::Info(
        "Initializing API client...".to_string(),
    ));
    let client = match ApiClient::new().await {
        Ok(c) => {
            let _ = tx.send(DistributionMessage::Success(
                "API client initialized".to_string(),
            ));
            c
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "ApiClient error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    };

    let _ = tx.send(DistributionMessage::Info(
        "Connecting to Elements RPC...".to_string(),
    ));
    let elements_rpc = match ElementsRpc::from_env() {
        Ok(e) => {
            let _ = tx.send(DistributionMessage::Success(
                "Elements RPC connected".to_string(),
            ));
            e
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Elements RPC error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    };

    let _ = tx.send(DistributionMessage::Info(
        "Generating signer...".to_string(),
    ));
    let signer = match LwkSoftwareSigner::generate_new_indexed(300) {
        Ok((_mn, s)) => {
            let _ = tx.send(DistributionMessage::Success("Signer generated".to_string()));
            s
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!("Signer error: {}", e)));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    };

    // 1. Validate GAID
    let _ = tx.send(DistributionMessage::Info(format!(
        "Validating GAID: {}...",
        gaid
    )));
    match client.validate_gaid(&gaid).await {
        Ok(v) if v.is_valid => {
            let _ = tx.send(DistributionMessage::Success("GAID is valid".to_string()));
        }
        Ok(_) => {
            let _ = tx.send(DistributionMessage::Error("GAID is invalid".to_string()));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "GAID validation error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    }

    // 2. Get GAID address
    let _ = tx.send(DistributionMessage::Info(
        "Fetching GAID address...".to_string(),
    ));
    let user_address = match client.get_gaid_address(&gaid).await {
        Ok(resp) if !resp.address.is_empty() => {
            let _ = tx.send(DistributionMessage::Success(format!(
                "Address: {}...{}",
                &resp.address[..10],
                &resp.address[resp.address.len() - 6..]
            )));
            resp.address
        }
        Ok(_) => {
            let _ = tx.send(DistributionMessage::Error(
                "No address associated with GAID".to_string(),
            ));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Address lookup error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    };

    // 3. Ensure registered user exists
    let _ = tx.send(DistributionMessage::Info(
        "Checking for registered user...".to_string(),
    ));
    let (user_id, _user_name) = match client.get_registered_users().await {
        Ok(users) => {
            if let Some(user) = users.iter().find(|u| u.gaid.as_ref() == Some(&gaid)) {
                let _ = tx.send(DistributionMessage::Success(format!(
                    "Found user '{}' (ID: {})",
                    user.name, user.id
                )));
                (user.id, user.name.clone())
            } else {
                let _ = tx.send(DistributionMessage::Info(
                    "User not found, creating...".to_string(),
                ));
                let name = format!("TUI Distribution User {}", chrono::Utc::now().timestamp());
                let req = amp_rs::model::RegisteredUserAdd {
                    name: name.clone(),
                    gaid: Some(gaid.clone()),
                    is_company: false,
                };
                match client.add_registered_user(&req).await {
                    Ok(u) => {
                        let _ = tx.send(DistributionMessage::Success(format!(
                            "Created user '{}' (ID: {})",
                            name, u.id
                        )));
                        (u.id, name)
                    }
                    Err(e) => {
                        let _ = tx.send(DistributionMessage::Error(format!(
                            "Register user error: {}",
                            e
                        )));
                        let _ = tx.send(DistributionMessage::Complete);
                        return;
                    }
                }
            }
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Failed to list users: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    };

    // 4. Ensure category exists and associations
    let _ = tx.send(DistributionMessage::Info(
        "Setting up category...".to_string(),
    ));
    let category_id = match client.get_categories().await {
        Ok(list) => {
            if let Some(c) = list.into_iter().find(|c| c.name == DEMO_CATEGORY_NAME) {
                let _ = tx.send(DistributionMessage::Success(format!(
                    "Category '{}' exists",
                    DEMO_CATEGORY_NAME
                )));
                c.id
            } else {
                let _ = tx.send(DistributionMessage::Info(
                    "Creating category...".to_string(),
                ));
                match client
                    .add_category(&amp_rs::model::CategoryAdd {
                        name: DEMO_CATEGORY_NAME.to_string(),
                        description: Some("Demo category".to_string()),
                    })
                    .await
                {
                    Ok(c) => {
                        let _ = tx.send(DistributionMessage::Success(format!(
                            "Created category (ID: {})",
                            c.id
                        )));
                        c.id
                    }
                    Err(e) => {
                        let _ = tx.send(DistributionMessage::Error(format!(
                            "Create category error: {}",
                            e
                        )));
                        let _ = tx.send(DistributionMessage::Complete);
                        return;
                    }
                }
            }
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Get categories error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    };

    // Associate user and asset with category
    let _ = tx.send(DistributionMessage::Info(
        "Adding user to category...".to_string(),
    ));
    match client
        .add_registered_user_to_category(category_id, user_id)
        .await
    {
        Ok(_) => {
            let _ = tx.send(DistributionMessage::Success(
                "User added to category".to_string(),
            ));
        }
        Err(e) if format!("{}", e).contains("already") => {
            let _ = tx.send(DistributionMessage::Success(
                "User already in category".to_string(),
            ));
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Add user to category error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    }

    let _ = tx.send(DistributionMessage::Info(
        "Adding asset to category...".to_string(),
    ));
    match client.add_asset_to_category(category_id, asset_uuid).await {
        Ok(_) => {
            let _ = tx.send(DistributionMessage::Success(
                "Asset added to category".to_string(),
            ));
        }
        Err(e) if format!("{}", e).contains("already") => {
            let _ = tx.send(DistributionMessage::Success(
                "Asset already in category".to_string(),
            ));
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Add asset to category error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    }

    // 5. Get asset details for precision
    let _ = tx.send(DistributionMessage::Info(
        "Fetching asset precision...".to_string(),
    ));
    let asset = match client.get_asset(asset_uuid).await {
        Ok(a) => {
            let _ = tx.send(DistributionMessage::Success(format!(
                "Asset precision: {}",
                a.precision
            )));
            a
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Failed to fetch asset: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    };

    // 6. Create assignment
    let _ = tx.send(DistributionMessage::Info(
        "Creating assignment...".to_string(),
    ));
    let smallest_units = (amount_btc * 10f64.powi(asset.precision as i32)).round() as i64;
    let assignment_req = amp_rs::model::CreateAssetAssignmentRequest {
        registered_user: user_id,
        amount: smallest_units,
        vesting_timestamp: None,
        ready_for_distribution: true,
    };

    match client
        .create_asset_assignments(asset_uuid, &vec![assignment_req])
        .await
    {
        Ok(assignments) => {
            let _ = tx.send(DistributionMessage::Success(format!(
                "Created assignment for {} units",
                smallest_units
            )));
            if let Some(first) = assignments.first() {
                let _ = tx.send(DistributionMessage::Info(format!(
                    "Assignment ID: {}",
                    first.id
                )));
            }
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Create assignment error: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Complete);
            return;
        }
    }

    // 7. DRY RUN: Distribution (commented out to prevent UI freeze)
    // Convert smallest_units back to base unit (like BTC) using asset precision
    let amount_for_distribution = smallest_units as f64 / 10f64.powi(asset.precision as i32);

    // 7. Execute distribution (LIVE) - Now working with async background task!
    let distribution_assignments = vec![amp_rs::model::AssetDistributionAssignment {
        user_id: user_id.to_string(),
        address: user_address.clone(),
        amount: amount_for_distribution,
    }];

    let _ = tx.send(DistributionMessage::Info(format!(
        "Distributing {} base units ({} smallest units)...",
        amount_for_distribution, smallest_units
    )));
    let wallet_name = "amp_elements_wallet_static_for_funding";

    match client
        .distribute_asset(
            asset_uuid,
            distribution_assignments,
            &elements_rpc,
            wallet_name,
            &signer,
        )
        .await
    {
        Ok(()) => {
            let _ = tx.send(DistributionMessage::Success(
                "Distribution completed successfully!".to_string(),
            ));
            let _ = tx.send(DistributionMessage::Info(
                "Asset has been distributed to the user".to_string(),
            ));
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!(
                "Distribution failed: {}",
                e
            )));
            let _ = tx.send(DistributionMessage::Info(
                "Check the error details above".to_string(),
            ));
        }
    }

    // Signal completion
    let _ = tx.send(DistributionMessage::Complete);

    // COMMENTED OUT - This blocks the UI thread waiting for blockchain confirmations
    // Uncomment only if you implement proper async handling with channels
    /*
    let distribution_assignments = vec![amp_rs::model::AssetDistributionAssignment {
        user_id: user_id.to_string(),
        address: user_address.clone(),
        amount: amount_for_distribution,
    }];

    let wallet_name = "amp_elements_wallet_static_for_funding";

    match client.distribute_asset(
        asset_uuid,
        distribution_assignments,
        &elements_rpc,
        wallet_name,
        &signer,
    ).await {
        Ok(()) => {
            let _ = tx.send(DistributionMessage::Success("Distribution completed successfully!".to_string()));
            let _ = tx.send(DistributionMessage::Info("Asset has been distributed to the user".to_string()));
        }
        Err(e) => {
            let _ = tx.send(DistributionMessage::Error(format!("Distribution failed: {}", e)));
            let _ = tx.send(DistributionMessage::Info("Check the error details above".to_string()));
        }
    }
    */
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a runtime to fetch async data
    let rt = Runtime::new()?;

    println!("Fetching asset data from AMP API...");
    let data = rt.block_on(async { fetch_asset_data().await })?;

    println!("Data fetched successfully! Launching TUI...");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize app state
    let mut app = AppState {
        screen: AppScreen::Main,
        asset_data: data,
        distribution_input: DistributionInput::new(),
        distribution_progress: DistributionProgress::new(),
        distribution_rx: None,
        is_reloading: false,
    };

    // Run app
    let res = run_app(&mut terminal, &mut app, &rt);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}
