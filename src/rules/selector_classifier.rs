#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorClass {
    /// `getByRole`, `getByText`, `getByLabel`, etc. — semantic locators
    Semantic,
    /// `getByTestId`, `data-testid=` — explicit test attributes
    TestId,
    /// `#id`, `input#name` — CSS ID selectors
    CssId,
    /// `.class`, `div.class` — CSS class selectors
    CssClass,
    /// `//div`, `xpath=` — XPath selectors
    XPath,
    /// `div > span`, `.foo .bar` — descendant/child chains
    ChainedClass,
    /// Selectors that don't match a specific anti-pattern
    Other,
}

/// Classifies a locator raw argument string by selector type.
pub fn classify_selector(raw: &str) -> SelectorClass {
    if raw.is_empty() {
        return SelectorClass::Other;
    }

    // XPath
    if raw.starts_with("//") || raw.starts_with(".//") || raw.starts_with("xpath=") {
        return SelectorClass::XPath;
    }

    // CSS ID — check first since # is a strong signal
    if raw.contains('#') {
        return SelectorClass::CssId;
    }

    // Count CSS class tokens (.foo is a class token)
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let class_tokens: Vec<&&str> = tokens.iter().filter(|t| t.contains('.')).collect();

    // Chained class: more than one class token or a combinator like >, +, ~
    let has_combinator = raw.contains(" > ") || raw.contains(" + ") || raw.contains(" ~ ");
    if has_combinator || class_tokens.len() > 1 {
        return SelectorClass::ChainedClass;
    }

    // TestId
    if raw.starts_with("[data-testid") || raw.contains("data-testid=") {
        return SelectorClass::TestId;
    }

    // CSS class (single) — starts with . or has . but not spaces/quotes/equals (e.g., div.active)
    if raw.starts_with('.')
        || (raw.contains('.')
            && !raw.contains(' ')
            && !raw.contains('=')
            && !raw.contains('"')
            && !raw.contains('\''))
    {
        return SelectorClass::CssClass;
    }

    SelectorClass::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_xpath() {
        assert_eq!(classify_selector("//div[@id='app']"), SelectorClass::XPath);
        assert_eq!(classify_selector(".//span"), SelectorClass::XPath);
        assert_eq!(classify_selector("xpath=//button"), SelectorClass::XPath);
    }

    #[test]
    fn classifies_css_id() {
        assert_eq!(classify_selector("#submit-btn"), SelectorClass::CssId);
        assert_eq!(classify_selector("input#name"), SelectorClass::CssId);
        assert_eq!(classify_selector("button#primary"), SelectorClass::CssId);
    }

    #[test]
    fn classifies_chained_class() {
        assert_eq!(classify_selector(".foo .bar"), SelectorClass::ChainedClass);
        assert_eq!(
            classify_selector(".parent > .child"),
            SelectorClass::ChainedClass
        );
        assert_eq!(classify_selector("div .a .b"), SelectorClass::ChainedClass);
    }

    #[test]
    fn classifies_chained_class_plus_combinator() {
        // Single class token but a ` + ` combinator must still classify as
        // ChainedClass. Guards the `||` (not `&&`) in the combinator check.
        assert_eq!(classify_selector("div + span"), SelectorClass::ChainedClass);
        assert_eq!(
            classify_selector("label + input"),
            SelectorClass::ChainedClass
        );
    }

    #[test]
    fn classifies_chained_class_tilde_combinator() {
        // Single class token but a ` ~ ` general sibling combinator must still
        // classify as ChainedClass.
        assert_eq!(classify_selector("h1 ~ p"), SelectorClass::ChainedClass);
        assert_eq!(
            classify_selector("label ~ span"),
            SelectorClass::ChainedClass
        );
    }

    #[test]
    fn classifies_testid() {
        assert_eq!(
            classify_selector("[data-testid=\"submit\"]"),
            SelectorClass::TestId
        );
        assert_eq!(
            classify_selector("button[data-testid='ok']"),
            SelectorClass::TestId
        );
    }

    #[test]
    fn classifies_css_class() {
        assert_eq!(classify_selector(".btn-primary"), SelectorClass::CssClass);
        assert_eq!(classify_selector("div.active"), SelectorClass::CssClass);
    }

    #[test]
    fn classifies_other() {
        assert_eq!(classify_selector(""), SelectorClass::Other);
        assert_eq!(classify_selector("button"), SelectorClass::Other);
        assert_eq!(classify_selector("text=Submit"), SelectorClass::Other);
        assert_eq!(classify_selector(":visible"), SelectorClass::Other);
    }

    #[test]
    fn id_takes_precedence_over_chained() {
        assert_eq!(classify_selector("#foo .bar"), SelectorClass::CssId);
    }

    #[test]
    fn dotted_text_is_not_css_class() {
        assert_eq!(classify_selector("text=Mr. Smith"), SelectorClass::Other);
    }

    #[test]
    fn mixed_class_chains() {
        assert_eq!(
            classify_selector("div.foo .bar"),
            SelectorClass::ChainedClass
        );
    }
}
