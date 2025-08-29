use crate::graphics::context::GraphicsContext;

struct PageButton<CB> {
    label: String,
    callback: CB,
}

impl<CB> PageButton<CB>
where
    CB: FnMut(),
{
    pub fn new(label: String, callback: CB) -> Self {
        PageButton { label, callback }
    }

    pub fn trigger(&mut self) {
        (self.callback)();
    }
}

pub struct PageBase {
    id: u32,
    name: String,
    buttons: Vec<PageButton<Box<dyn FnMut()>>>,
}

trait Page {
    fn render(&self, context: &mut GraphicsContext);
}

pub struct PageManager {
    context: GraphicsContext,
    pages: Vec<Box<dyn Page>>,
}

impl PageManager {
    pub fn new(context: GraphicsContext) -> Self {
        PageManager { context, pages: Vec::new() }
    }
    pub fn add_page(&mut self, page: Box<dyn Page>) {
        self.pages.push(page);
    }
}