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

use amp_rs::ApiClient;
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
use tokio::runtime::Runtime;

// Demo asset information
const ASSET_UUID: &str = "004087c1-c213-4b5b-87a5-cd5b986ce08c";

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
    holders: Vec<(String, i64, Option<String>)>, // (owner address, amount, optional GAID)
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
        format!("{}.{:0width$}", whole, fractional, width = self.precision as usize)
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

    // Convert ownership data to holders list
    let holders: Vec<(String, i64, Option<String>)> = ownerships
        .into_iter()
        .map(|o| (o.owner, o.amount, o.gaid))
        .collect();

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

fn ui(f: &mut Frame, data: &AssetDisplayData) {
    let size = f.area();

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
            "Blockstream AMP Asset Display",
            Style::default().fg(Color::Gray),
        ),
    ])];

    let header = Paragraph::new(header_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Asset Information ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

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

    // Footer with instructions
    let footer_text = vec![Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::Gray)),
        Span::styled("'q'", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" or ", Style::default().fg(Color::Gray)),
        Span::styled("'Esc'", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" to exit", Style::default().fg(Color::Gray)),
    ])];

    let footer = Paragraph::new(footer_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);
}

fn render_asset_details(f: &mut Frame, area: Rect, data: &AssetDisplayData) {
    let details = vec![
        Line::from(vec![
            Span::styled("UUID: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(&data.asset_uuid),
        ]),
        Line::from(vec![
            Span::styled("Asset ID: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(&data.asset_id[..32]),
        ]),
        Line::from(vec![
            Span::raw("          "),
            Span::raw(&data.asset_id[32..]),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Ticker: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(&data.ticker, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Precision: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(data.precision.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Domain: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(&data.domain),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("  ● Registered: "),
            Span::styled(
                if data.is_registered { "Yes ✓" } else { "No ✗" },
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
                if data.is_authorized { "Yes ✓" } else { "No ✗" },
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
                if data.is_locked { "Yes 🔒" } else { "No 🔓" },
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
                if data.transfer_restricted { "Yes" } else { "No" },
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
                .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_circulation_stats(f: &mut Frame, area: Rect, data: &AssetDisplayData) {
    let circulation = data.calculate_circulation();
    let available = data.calculate_available();

    let stats = vec![
        Line::from(vec![
            Span::styled("Total Circulation: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(
                data.format_amount(circulation),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
        Line::from(vec![
            Span::styled("Distribution: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
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
        Line::from(vec![
            Span::styled("Special: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
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
        Line::from(vec![
            Span::styled("Users: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
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
                .title_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
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
            Span::styled("Total Holders: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(
                total_holders.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Total Held: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(
                data.format_amount(total_held),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![
            Span::styled("━".repeat(50), Style::default().fg(Color::Gray)),
        ])),
    ];

    let header_list = List::new(header_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta))
                .title(format!(" Asset Holders ({}) ", total_holders))
                .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
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
        let empty_msg = Paragraph::new(Line::from(vec![
            Span::styled(
                "No holders found",
                Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
            ),
        ]))
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
    for (idx, (i, (owner, amount, gaid))) in data.holders.iter().enumerate().take(holders_to_show).enumerate() {
        let percentage_of_supply = (*amount as f64 / total_circulation as f64) * 100.0;
        
        // Format owner address (truncate if too long)
        let owner_display = if owner.len() > 45 {
            format!("{}...{}", &owner[..20], &owner[owner.len()-20..])
        } else {
            owner.clone()
        };

        // Create holder info lines
        let mut holder_lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("#{} ", i + 1),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("{:.2}% of supply", percentage_of_supply),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ]),
        ];

        // Add address line
        holder_lines.push(Line::from(vec![
            Span::styled("Address: ", Style::default().fg(Color::Cyan)),
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
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]));

        let holder_info = Paragraph::new(holder_lines)
            .block(Block::default());

        // Split holder chunk into info and gauge
        let holder_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(if gaid.is_some() { 4 } else { 3 }), // Info lines
                Constraint::Length(1), // Gauge
                Constraint::Length(1), // Spacing
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
        let more_msg = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("... and {} more holder(s)", remaining),
                Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
            ),
        ]))
        .alignment(Alignment::Center);
        if let Some(last_chunk) = holder_chunks.last() {
            f.render_widget(more_msg, *last_chunk);
        }
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    data: &AssetDisplayData,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, data))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a runtime to fetch async data
    let rt = Runtime::new()?;
    
    println!("Fetching asset data from AMP API...");
    let data = rt.block_on(async {
        fetch_asset_data().await
    })?;
    
    println!("Data fetched successfully! Launching TUI...");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let res = run_app(&mut terminal, &data);

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
