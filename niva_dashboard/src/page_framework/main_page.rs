use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition};

pub struct MainPage {
    base: PageBase,
}

impl MainPage {
    pub fn new(id: u32, name: String, ui_style: UIStyle) -> Self {
        MainPage {
            base: PageBase::new(id, name, ui_style),
        }
    }

    pub fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }

    pub fn ui_style(&self) -> &UIStyle {
        self.base.ui_style()
    }
}

impl Page for MainPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn render(&self, context: &mut GraphicsContext) -> Result<(), String> {
        context.render_text_with_font(
            "Main Page", 
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
}