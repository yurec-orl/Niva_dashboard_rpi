use crate::graphics::context::GraphicsContext;
use crate::page_framework::page_manager::{PageManager, Page, PageBase};

pub struct MainPage {
    base: PageBase,
}

impl MainPage {
    pub fn new(id: u32, name: String) -> Self {
        MainPage {
            base: PageBase::new(id, name),
        }
    }
}

impl Page for MainPage {
    fn render(&self, context: &mut GraphicsContext) -> Result<(), String> {
        context.render_text("Main Page", 10.0, 10.0, 1.0, (1.0, 1.0, 1.0))?;
        Ok(())
    }

    fn on_enter(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_exit(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_button(&mut self, button: char) -> Result<(), String> {
        Ok(())
    }
}