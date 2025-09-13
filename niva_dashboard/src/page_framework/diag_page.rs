use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::events::{EventSender, EventReceiver, UIEvent};
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, MAIN_PAGE_ID};
use crate::hardware::sensor_manager::SensorManager;

pub struct DiagPage {
    base: PageBase,
    event_receiver: EventReceiver,
    event_sender: EventSender,
}

impl DiagPage {
    pub fn new(id: u32, ui_style: UIStyle, event_sender: EventSender, event_receiver: EventReceiver) -> Self {
        let mut diag_page = DiagPage {
            base: PageBase::new(id, "Diag".to_string(), ui_style),
            event_sender,
            event_receiver,
        };

        diag_page.setup_buttons();
        
        diag_page
    }

    pub fn setup_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ДАТЧ".into(), Box::new({
                let sender = self.event_sender.clone();
                move || sender.send(UIEvent::ButtonPressed("diag_test_1".into()))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ЖУРН".into(), Box::new({
                let sender = self.event_sender.clone();
                move || sender.send(UIEvent::ButtonPressed("diag_test_2".into()))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ВОЗВ".into(), Box::new({
                let sender = self.event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(MAIN_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }
}

impl Page for DiagPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }

    fn render(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager) -> Result<(), String> {
        context.render_text_with_font(
            "Diagnostics Page", 
            200.0, 
            100.0, 
            1.0, 
            self.ui_style().get_color(TEXT_PRIMARY_COLOR, (1.0, 1.0, 1.0)),
            &self.ui_style().get_string(TEXT_PRIMARY_FONT, "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
            self.ui_style().get_integer(TEXT_PRIMARY_FONT_SIZE, 24)
        )?;
        Ok(())
    }

    fn on_enter(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_exit(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_button(&mut self, _button: char) -> Result<(), String> {
        Ok(())
    }

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        self.base.buttons()
    }

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position(pos)
    }

    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position_mut(pos)
    }

    fn ui_style(&self) -> &UIStyle {
        self.base.ui_style()
    }
}