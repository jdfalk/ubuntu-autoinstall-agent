// file: src/commands/editor.rs
// version: 1.0.0
// guid: 0c1d2e3f-4a5b-6c7d-8e9f-0a1b2c3d4e5f

use crate::executor::Executor;
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Build the editor command
pub fn build_command() -> Command {
    Command::new("editor")
        .about("Superior Rust-powered text editor")
        .alias("edit")
        .alias("vi")
        .alias("vim")
        .alias("nano")
        .arg(Arg::new("file")
            .help("File to edit")
            .required(true))
        .arg(Arg::new("line")
            .help("Start at line number")
            .short('l')
            .long("line")
            .value_name("NUMBER"))
        .arg(Arg::new("column")
            .help("Start at column number")
            .short('c')
            .long("column")
            .value_name("NUMBER"))
        .arg(Arg::new("readonly")
            .help("Open in read-only mode")
            .short('r')
            .long("readonly")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("syntax")
            .help("Syntax highlighting language")
            .short('s')
            .long("syntax")
            .value_name("LANG"))
}

/// Execute the custom Rust editor
pub async fn execute(matches: &ArgMatches, _executor: &Executor) -> Result<()> {
    let file_path = matches.get_one::<String>("file").unwrap();
    let start_line = matches.get_one::<String>("line")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);
    let start_column = matches.get_one::<String>("column")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);
    let readonly = matches.get_flag("readonly");
    let syntax_lang = matches.get_one::<String>("syntax");

    // Initialize the editor
    let mut editor = RustEditor::new(file_path, readonly, syntax_lang)?;
    editor.set_cursor_position(start_line.saturating_sub(1), start_column.saturating_sub(1));

    // Run the editor
    editor.run().await
}

struct RustEditor {
    file_path: String,
    content: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    scroll_offset: usize,
    mode: EditorMode,
    status_message: String,
    modified: bool,
    readonly: bool,
    syntax_lang: Option<String>,
    search_query: Option<String>,
    clipboard: String,
}

#[derive(Debug, PartialEq)]
enum EditorMode {
    Normal,
    Insert,
    Command,
    Search,
    Visual,
}

impl RustEditor {
    fn new(file_path: &str, readonly: bool, syntax_lang: Option<&String>) -> Result<Self> {
        let content = if Path::new(file_path).exists() {
            fs::read_to_string(file_path)?
                .lines()
                .map(|line| line.to_string())
                .collect()
        } else {
            vec![String::new()]
        };

        Ok(Self {
            file_path: file_path.to_string(),
            content,
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            mode: EditorMode::Normal,
            status_message: format!("Opened: {}", file_path),
            modified: false,
            readonly,
            syntax_lang: syntax_lang.cloned(),
            search_query: None,
            clipboard: String::new(),
        })
    }

    fn set_cursor_position(&mut self, line: usize, col: usize) {
        self.cursor_line = line.min(self.content.len().saturating_sub(1));
        self.cursor_col = col.min(self.get_current_line().len());
    }

    async fn run(&mut self) -> Result<()> {
        // Enter alternate screen and enable raw mode
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;

        let result = self.editor_loop().await;

        // Cleanup
        execute!(io::stdout(), LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;

        result
    }

    async fn editor_loop(&mut self) -> Result<()> {
        loop {
            self.draw_screen()?;

            if let Event::Key(key_event) = event::read()? {
                if self.handle_key_event(key_event)? {
                    break; // Exit requested
                }
            }
        }

        Ok(())
    }

    fn draw_screen(&self) -> Result<()> {
        let (cols, rows) = terminal::size()?;
        let status_row = rows - 1;

        // Clear screen
        execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;

        // Draw content
        for screen_row in 0..status_row {
            let content_row = screen_row as usize + self.scroll_offset;

            if content_row < self.content.len() {
                let line = &self.content[content_row];
                self.draw_line_with_syntax(line, screen_row, cols)?;
            } else {
                // Empty line indicator
                execute!(
                    io::stdout(),
                    MoveTo(0, screen_row),
                    SetForegroundColor(Color::DarkGrey),
                    Print("~"),
                    ResetColor
                )?;
            }
        }

        // Draw status line
        self.draw_status_line(status_row, cols)?;

        // Position cursor
        let screen_cursor_row = (self.cursor_line - self.scroll_offset) as u16;
        let screen_cursor_col = self.cursor_col as u16;
        execute!(io::stdout(), MoveTo(screen_cursor_col, screen_cursor_row))?;

        io::stdout().flush()?;
        Ok(())
    }

    fn draw_line_with_syntax(&self, line: &str, row: u16, _cols: u16) -> Result<()> {
        execute!(io::stdout(), MoveTo(0, row))?;

        if let Some(lang) = &self.syntax_lang {
            self.draw_with_syntax_highlighting(line, lang)?;
        } else {
            // Basic syntax highlighting based on file extension
            let ext = Path::new(&self.file_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            match ext {
                "rs" => self.draw_rust_syntax(line)?,
                "py" => self.draw_python_syntax(line)?,
                "js" | "ts" => self.draw_javascript_syntax(line)?,
                "go" => self.draw_go_syntax(line)?,
                _ => execute!(io::stdout(), Print(line))?,
            }
        }

        Ok(())
    }

    fn draw_rust_syntax(&self, line: &str) -> Result<()> {
        let keywords = [
            "fn", "let", "mut", "const", "static", "struct", "enum", "impl", "trait",
            "pub", "use", "mod", "crate", "super", "self", "Self", "match", "if", "else",
            "for", "while", "loop", "break", "continue", "return", "async", "await",
        ];

        self.highlight_keywords(line, &keywords, Color::Blue)?;
        Ok(())
    }

    fn draw_python_syntax(&self, line: &str) -> Result<()> {
        let keywords = [
            "def", "class", "if", "elif", "else", "for", "while", "try", "except",
            "finally", "with", "as", "import", "from", "return", "yield", "pass",
            "break", "continue", "lambda", "and", "or", "not", "in", "is",
        ];

        self.highlight_keywords(line, &keywords, Color::Blue)?;
        Ok(())
    }

    fn draw_javascript_syntax(&self, line: &str) -> Result<()> {
        let keywords = [
            "function", "var", "let", "const", "if", "else", "for", "while", "do",
            "switch", "case", "default", "break", "continue", "return", "try", "catch",
            "finally", "throw", "new", "this", "class", "extends", "async", "await",
        ];

        self.highlight_keywords(line, &keywords, Color::Blue)?;
        Ok(())
    }

    fn draw_go_syntax(&self, line: &str) -> Result<()> {
        let keywords = [
            "func", "var", "const", "type", "struct", "interface", "if", "else",
            "for", "range", "switch", "case", "default", "break", "continue",
            "return", "go", "defer", "select", "chan", "make", "new",
        ];

        self.highlight_keywords(line, &keywords, Color::Blue)?;
        Ok(())
    }

    fn draw_with_syntax_highlighting(&self, line: &str, _lang: &str) -> Result<()> {
        // Custom syntax highlighting based on language
        execute!(io::stdout(), Print(line))?;
        Ok(())
    }

    fn highlight_keywords(&self, line: &str, keywords: &[&str], color: Color) -> Result<()> {
        let mut pos = 0;
        let chars: Vec<char> = line.chars().collect();

        while pos < chars.len() {
            let mut found_keyword = false;

            // Check for keywords
            for &keyword in keywords {
                if self.matches_keyword_at(&chars, pos, keyword) {
                    // Print preceding text
                    if pos > 0 {
                        let preceding: String = chars[..pos].iter().collect();
                        execute!(io::stdout(), Print(preceding))?;
                        pos = 0;
                        break;
                    }

                    // Print keyword with color
                    execute!(
                        io::stdout(),
                        SetForegroundColor(color),
                        Print(keyword),
                        ResetColor
                    )?;

                    pos += keyword.len();
                    found_keyword = true;
                    break;
                }
            }

            if !found_keyword {
                pos += 1;
            }
        }

        // Print remaining text
        if pos < chars.len() {
            let remaining: String = chars[pos..].iter().collect();
            execute!(io::stdout(), Print(remaining))?;
        } else if pos == 0 {
            execute!(io::stdout(), Print(line))?;
        }

        Ok(())
    }

    fn matches_keyword_at(&self, chars: &[char], pos: usize, keyword: &str) -> bool {
        let keyword_chars: Vec<char> = keyword.chars().collect();

        if pos + keyword_chars.len() > chars.len() {
            return false;
        }

        // Check if characters match
        for (i, &kw_char) in keyword_chars.iter().enumerate() {
            if chars[pos + i] != kw_char {
                return false;
            }
        }

        // Check word boundaries
        let before_ok = pos == 0 || !chars[pos - 1].is_alphanumeric();
        let after_pos = pos + keyword_chars.len();
        let after_ok = after_pos == chars.len() || !chars[after_pos].is_alphanumeric();

        before_ok && after_ok
    }

    fn draw_status_line(&self, row: u16, cols: u16) -> Result<()> {
        execute!(
            io::stdout(),
            MoveTo(0, row),
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(Color::White)
        )?;

        let mode_str = match self.mode {
            EditorMode::Normal => "NORMAL",
            EditorMode::Insert => "INSERT",
            EditorMode::Command => "COMMAND",
            EditorMode::Search => "SEARCH",
            EditorMode::Visual => "VISUAL",
        };

        let left_status = format!(
            " {} | {} | {}:{} ",
            mode_str,
            if self.modified { "[+]" } else { "   " },
            self.cursor_line + 1,
            self.cursor_col + 1
        );

        let right_status = format!(
            " {} | {}/{} ",
            Path::new(&self.file_path).file_name().unwrap_or_default().to_string_lossy(),
            self.cursor_line + 1,
            self.content.len()
        );

        // Print left status
        execute!(io::stdout(), Print(&left_status))?;

        // Fill middle with spaces
        let used_space = left_status.len() + right_status.len();
        let remaining_space = cols as usize - used_space.min(cols as usize);
        execute!(io::stdout(), Print(" ".repeat(remaining_space)))?;

        // Print right status
        execute!(io::stdout(), Print(&right_status))?;

        execute!(io::stdout(), ResetColor)?;
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        match self.mode {
            EditorMode::Normal => self.handle_normal_mode(key_event),
            EditorMode::Insert => self.handle_insert_mode(key_event),
            EditorMode::Command => self.handle_command_mode(key_event),
            EditorMode::Search => self.handle_search_mode(key_event),
            EditorMode::Visual => self.handle_visual_mode(key_event),
        }
    }

    fn handle_normal_mode(&mut self, key_event: KeyEvent) -> Result<bool> {
        match key_event.code {
            KeyCode::Char('q') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.modified {
                    self.status_message = "File has unsaved changes! Use Ctrl+Q again to force quit.".to_string();
                    return Ok(false);
                }
                return Ok(true); // Exit
            }
            KeyCode::Char('s') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_file()?;
            }
            KeyCode::Char('i') => {
                self.mode = EditorMode::Insert;
                self.status_message = "-- INSERT --".to_string();
            }
            KeyCode::Char('a') => {
                self.mode = EditorMode::Insert;
                self.move_cursor_right();
                self.status_message = "-- INSERT --".to_string();
            }
            KeyCode::Char('o') => {
                self.mode = EditorMode::Insert;
                self.insert_line_below();
                self.status_message = "-- INSERT --".to_string();
            }
            KeyCode::Char('O') => {
                self.mode = EditorMode::Insert;
                self.insert_line_above();
                self.status_message = "-- INSERT --".to_string();
            }
            KeyCode::Char(':') => {
                self.mode = EditorMode::Command;
                self.status_message = ":".to_string();
            }
            KeyCode::Char('/') => {
                self.mode = EditorMode::Search;
                self.status_message = "/".to_string();
            }
            KeyCode::Char('v') => {
                self.mode = EditorMode::Visual;
                self.status_message = "-- VISUAL --".to_string();
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_cursor_left(),
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor_up(),
            KeyCode::Char('l') | KeyCode::Right => self.move_cursor_right(),
            KeyCode::Char('w') => self.move_word_forward(),
            KeyCode::Char('b') => self.move_word_backward(),
            KeyCode::Char('0') => self.move_to_line_start(),
            KeyCode::Char('$') => self.move_to_line_end(),
            KeyCode::Char('g') => self.move_to_file_start(),
            KeyCode::Char('G') => self.move_to_file_end(),
            KeyCode::Char('x') => self.delete_char(),
            KeyCode::Char('d') => self.delete_line(),
            KeyCode::Char('y') => self.yank_line(),
            KeyCode::Char('p') => self.paste(),
            KeyCode::Char('u') => self.undo(),
            _ => {}
        }

        self.adjust_scroll();
        Ok(false)
    }

    fn handle_insert_mode(&mut self, key_event: KeyEvent) -> Result<bool> {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.status_message = String::new();
            }
            KeyCode::Char(c) => {
                self.insert_char(c);
            }
            KeyCode::Enter => {
                self.insert_newline();
            }
            KeyCode::Backspace => {
                self.delete_char_before_cursor();
            }
            KeyCode::Tab => {
                self.insert_char('\t');
            }
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Down => self.move_cursor_down(),
            _ => {}
        }

        self.adjust_scroll();
        Ok(false)
    }

    fn handle_command_mode(&mut self, key_event: KeyEvent) -> Result<bool> {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.status_message = String::new();
            }
            KeyCode::Enter => {
                let result = self.execute_command();
                self.mode = EditorMode::Normal;
                return result;
            }
            KeyCode::Char(c) => {
                self.status_message.push(c);
            }
            KeyCode::Backspace => {
                if self.status_message.len() > 1 {
                    self.status_message.pop();
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_search_mode(&mut self, key_event: KeyEvent) -> Result<bool> {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.status_message = String::new();
            }
            KeyCode::Enter => {
                let query = self.status_message[1..].to_string();
                self.search_query = Some(query.clone());
                self.search_forward(&query);
                self.mode = EditorMode::Normal;
                self.status_message = format!("Found: {}", query);
            }
            KeyCode::Char(c) => {
                self.status_message.push(c);
            }
            KeyCode::Backspace => {
                if self.status_message.len() > 1 {
                    self.status_message.pop();
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_visual_mode(&mut self, key_event: KeyEvent) -> Result<bool> {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.status_message = String::new();
            }
            KeyCode::Char('y') => {
                self.yank_line();
                self.mode = EditorMode::Normal;
                self.status_message = "Yanked line".to_string();
            }
            KeyCode::Char('d') => {
                self.delete_line();
                self.mode = EditorMode::Normal;
                self.status_message = "Deleted line".to_string();
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_cursor_left(),
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor_up(),
            KeyCode::Char('l') | KeyCode::Right => self.move_cursor_right(),
            _ => {}
        }

        self.adjust_scroll();
        Ok(false)
    }

    fn execute_command(&mut self) -> Result<bool> {
        let command = self.status_message[1..].to_string(); // Remove the ':' and clone

        match command.as_str() {
            "q" | "quit" => {
                if self.modified {
                    self.status_message = "File has unsaved changes! Use :q! to force quit.".to_string();
                    return Ok(false);
                }
                return Ok(true);
            }
            "q!" | "quit!" => return Ok(true),
            "w" | "write" => {
                self.save_file()?;
            }
            "wq" | "x" => {
                self.save_file()?;
                return Ok(true);
            }
            _ if command.starts_with("w ") => {
                let new_path = &command[2..];
                self.save_file_as(new_path)?;
            }
            _ => {
                self.status_message = format!("Unknown command: {}", command);
            }
        }

        Ok(false)
    }

    // Editor operations
    fn get_current_line(&self) -> &str {
        self.content.get(self.cursor_line).map(|s| s.as_str()).unwrap_or("")
    }

    fn get_current_line_mut(&mut self) -> &mut String {
        &mut self.content[self.cursor_line]
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.get_current_line().len();
        }
    }

    fn move_cursor_right(&mut self) {
        let line_len = self.get_current_line().len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_line < self.content.len() - 1 {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let line_len = self.get_current_line().len();
            self.cursor_col = self.cursor_col.min(line_len);
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor_line < self.content.len() - 1 {
            self.cursor_line += 1;
            let line_len = self.get_current_line().len();
            self.cursor_col = self.cursor_col.min(line_len);
        }
    }

    fn move_word_forward(&mut self) {
        let line = self.get_current_line();
        let chars: Vec<char> = line.chars().collect();

        while self.cursor_col < chars.len() && !chars[self.cursor_col].is_alphanumeric() {
            self.cursor_col += 1;
        }

        while self.cursor_col < chars.len() && chars[self.cursor_col].is_alphanumeric() {
            self.cursor_col += 1;
        }
    }

    fn move_word_backward(&mut self) {
        if self.cursor_col == 0 {
            return;
        }

        let line = self.get_current_line();
        let chars: Vec<char> = line.chars().collect();

        self.cursor_col -= 1;

        while self.cursor_col > 0 && !chars[self.cursor_col].is_alphanumeric() {
            self.cursor_col -= 1;
        }

        while self.cursor_col > 0 && chars[self.cursor_col - 1].is_alphanumeric() {
            self.cursor_col -= 1;
        }
    }

    fn move_to_line_start(&mut self) {
        self.cursor_col = 0;
    }

    fn move_to_line_end(&mut self) {
        self.cursor_col = self.get_current_line().len();
    }

    fn move_to_file_start(&mut self) {
        self.cursor_line = 0;
        self.cursor_col = 0;
    }

    fn move_to_file_end(&mut self) {
        self.cursor_line = self.content.len().saturating_sub(1);
        self.cursor_col = self.get_current_line().len();
    }

    fn insert_char(&mut self, c: char) {
        let cursor_col = self.cursor_col;
        let line = self.get_current_line_mut();
        line.insert(cursor_col, c);
        self.cursor_col += 1;
        self.modified = true;
    }

    fn insert_newline(&mut self) {
        let cursor_col = self.cursor_col;
        let current_line = self.get_current_line();
        let split_pos = std::cmp::min(cursor_col, current_line.len());
        let left = current_line[..split_pos].to_string();
        let right = current_line[split_pos..].to_string();

        self.content[self.cursor_line] = left;
        self.content.insert(self.cursor_line + 1, right);

        self.cursor_line += 1;
        self.cursor_col = 0;
        self.modified = true;
    }

    fn delete_char_before_cursor(&mut self) {
        if self.cursor_col > 0 {
            let cursor_col = self.cursor_col;
            let line = self.get_current_line_mut();
            line.remove(cursor_col - 1);
            self.cursor_col -= 1;
            self.modified = true;
        } else if self.cursor_line > 0 {
            // Join with previous line
            let current_line = self.content.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_line_len = self.get_current_line().len();
            self.cursor_col = prev_line_len;
            self.get_current_line_mut().push_str(&current_line);
            self.modified = true;
        }
    }

    fn delete_char(&mut self) {
        let cursor_col = self.cursor_col;
        let line = self.get_current_line_mut();
        if cursor_col < line.len() {
            line.remove(cursor_col);
            self.modified = true;
        }
    }

    fn delete_line(&mut self) {
        if self.content.len() > 1 {
            self.clipboard = self.content.remove(self.cursor_line);
            if self.cursor_line >= self.content.len() {
                self.cursor_line = self.content.len() - 1;
            }
        } else {
            self.clipboard = self.content[0].clone();
            self.content[0].clear();
        }
        self.cursor_col = 0;
        self.modified = true;
    }

    fn yank_line(&mut self) {
        self.clipboard = self.get_current_line().to_string();
    }

    fn paste(&mut self) {
        self.content.insert(self.cursor_line + 1, self.clipboard.clone());
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.modified = true;
    }

    fn insert_line_below(&mut self) {
        self.content.insert(self.cursor_line + 1, String::new());
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.modified = true;
    }

    fn insert_line_above(&mut self) {
        self.content.insert(self.cursor_line, String::new());
        self.cursor_col = 0;
        self.modified = true;
    }

    fn undo(&mut self) {
        // Simplified undo - just show message
        self.status_message = "Undo not implemented yet".to_string();
    }

    fn search_forward(&mut self, query: &str) {
        for (line_idx, line) in self.content.iter().enumerate().skip(self.cursor_line) {
            if let Some(col_idx) = line.find(query) {
                self.cursor_line = line_idx;
                self.cursor_col = col_idx;
                return;
            }
        }

        // Search from beginning
        for (line_idx, line) in self.content.iter().enumerate() {
            if line_idx >= self.cursor_line {
                break;
            }
            if let Some(col_idx) = line.find(query) {
                self.cursor_line = line_idx;
                self.cursor_col = col_idx;
                return;
            }
        }
    }

    fn adjust_scroll(&mut self) {
        let (_, rows) = terminal::size().unwrap_or((80, 24));
        let visible_rows = rows as usize - 1; // Reserve one row for status

        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.cursor_line - visible_rows + 1;
        }
    }

    fn save_file(&mut self) -> Result<()> {
        if self.readonly {
            self.status_message = "File is read-only!".to_string();
            return Ok(());
        }

        let content = self.content.join("\n");
        fs::write(&self.file_path, content)?;
        self.modified = false;
        self.status_message = format!("Saved: {}", self.file_path);
        Ok(())
    }

    fn save_file_as(&mut self, path: &str) -> Result<()> {
        if self.readonly {
            self.status_message = "File is read-only!".to_string();
            return Ok(());
        }

        let content = self.content.join("\n");
        fs::write(path, content)?;
        self.file_path = path.to_string();
        self.modified = false;
        self.status_message = format!("Saved as: {}", path);
        Ok(())
    }
}
