use dom::node::AbstractNode;
use newcss::complete::CompleteSelectResults;

use core::cast::transmute;

pub trait NodeUtil {
    fn get_css_select_results(self) -> &self/CompleteSelectResults;
    fn set_css_select_results(self, decl: CompleteSelectResults);
}

impl NodeUtil for AbstractNode {
    /** 
     * Provides the computed style for the given node. If CSS selector
     * Returns the style results for the given node. If CSS selector
     * matching has not yet been performed, fails.
     * FIXME: This isn't completely memory safe since the style is
     * stored in a box that can be overwritten
     */
    fn get_css_select_results(self) -> &self/CompleteSelectResults {
        if !self.has_layout_data() {
            fail!(~"style() called on a node without aux data!");
        }

        match self.layout_data().style {
            None => fail!(~"style() called on node without a style!"),
            Some(ref style) => unsafe { transmute(style) }
        }
    }

    /// Update the computed style of an HTML element with a style specified by CSS.
    fn set_css_select_results(self, decl: CompleteSelectResults) {
        if !self.has_layout_data() {
            fail!(~"set_css_select_results() called on a node without aux data!");
        }

        self.layout_data().style = Some(decl);
    }
}
