use crate::graphics::context::GraphicsContext;
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition};

pub struct MainPage {
    base: PageBase,
}

impl MainPage {
    pub fn new(id: u32, name: String) -> Self {
        MainPage {
            base: PageBase::new(id, name),
        }
    }

    pub fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }
}

impl Page for MainPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn render(&self, context: &mut GraphicsContext) -> Result<(), String> {
        context.render_text("Main Page", 100.0, 100.0, 1.0, (1.0, 1.0, 1.0))?;
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