// Example showing the benefits of the new shared reference approach

use std::rc::Rc;
use std::cell::RefCell;

// This is how you would use the new shared reference system:

fn main() {
    let mut page_manager = PageManager::new(/* context */);
    
    // OLD WAY (copying - inefficient):
    // let main_page = MainPage::new();
    // page_manager.add_page(main_page); // This would copy the entire struct
    // page_manager.switch_page_by_index(0); // Switch causes another copy
    
    // NEW WAY (shared references - efficient):
    let main_page = MainPage::new();
    let page_ref = page_manager.add_page(main_page); // Returns Rc<RefCell<MainPage>>
    
    // No copying happens - just reference counting
    page_manager.set_current_page(page_ref.clone()); // Clone just increments reference count
    
    // Multiple references to the same page data
    let another_ref = page_ref.clone(); // Still the same data, no copying
    
    // Access the page data when needed
    {
        let page = page_ref.borrow(); // Borrow for reading
        println!("Page title: {}", page.get_title());
    } // Borrow ends here
    
    {
        let mut page = page_ref.borrow_mut(); // Borrow for writing
        page.update_something();
    } // Mutable borrow ends here
    
    // The page manager can render without any copying
    page_manager.render(); // Uses current_page.borrow() internally
}

// Benefits of the new approach:
// 1. No copying of large page structures
// 2. Multiple references to the same page data
// 3. Runtime borrow checking ensures memory safety
// 4. Clean API that feels natural in Rust
// 5. Efficient page switching (just changing references)
