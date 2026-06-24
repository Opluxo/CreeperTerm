use iced::widget::{button, container, horizontal_space, row, text};
use iced::{Element, Length};

#[derive(Debug, Clone)]
pub struct TabBar {
    tabs: Vec<Tab>,
    active_tab: usize,
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: usize,
    pub title: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabClicked(usize),
    NewTab,
    #[allow(dead_code)]
    CloseTab(usize),
}

impl TabBar {
    pub fn new() -> Self {
        Self {
            tabs: vec![Tab {
                id: 1,
                title: "Terminal 1".to_string(),
            }],
            active_tab: 0,
        }
    }

    pub fn update(&mut self, message: Message) -> Option<usize> {
        match message {
            Message::TabClicked(index) => {
                if index < self.tabs.len() {
                    self.active_tab = index;
                    Some(self.tabs[index].id)
                } else {
                    None
                }
            }
            Message::NewTab => {
                let id = self.tabs.len() + 1;
                self.tabs.push(Tab {
                    id,
                    title: format!("Terminal {}", id),
                });
                self.active_tab = self.tabs.len() - 1;
                Some(id)
            }
            Message::CloseTab(index) => {
                if self.tabs.len() > 1 && index < self.tabs.len() {
                    self.tabs.remove(index);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                    Some(self.tabs[self.active_tab].id)
                } else {
                    None
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let mut tab_row = row![];

        for (index, tab) in self.tabs.iter().enumerate() {
            let title = text(&tab.title).size(14);
            let tab_button = button(title).on_press(Message::TabClicked(index));
            tab_row = tab_row.push(tab_button);
        }

        let new_tab_button = button(text("+").size(14)).on_press(Message::NewTab);

        tab_row = tab_row.push(horizontal_space());
        tab_row = tab_row.push(new_tab_button);

        container(tab_row)
            .width(Length::Fill)
            .padding(4)
            .into()
    }

    #[allow(dead_code)]
    pub fn active_tab_id(&self) -> usize {
        self.tabs[self.active_tab].id
    }
}
