use crate::compiler::{
    codegen::{
        HEML_ID_ATTRIBUTE_KEY,
        types::{
            Compiler, HtmlEventList, JSGenerator,
            minify::{self, None},
        },
        util,
    },
    obfuscation::ObfuscatedExpr,
};
use indoc::indoc;
use std::fmt::Write;

impl JSGenerator for minify::None {
    fn write_js_element_event(
        out_event_binding: &mut String,
        out_event_element_decleration: &mut String,
        events: HtmlEventList,
    ) {
        let _ = writeln!(
            out_event_element_decleration,
            "\tconst elm_{0} = frag.querySelector('[{1}=\"{0}\"]');",
            events.id, HEML_ID_ATTRIBUTE_KEY,
        );

        for event in events.iter() {
            let _ = writeln!(
                out_event_binding,
                "\telm_{}.addEventListener('{}',{} (event) => {{\n\t\t{}\n    }});",
                events.id,
                event.event_name,
                if event.is_async { " async" } else { "" },
                event.event_body.trim()
            );
        }
    }

    fn write_global_scope(out: &mut String) {
        let _ = write!(out, "\tconst frag = document;");
    }

    fn write_expr_bind(out: &mut String, var: &str, deps_array: &str, expr: &str) {
        let _ = write!(
            out,
            "BindExpression(FindMarker('{}', frag), {}, () => ({}));",
            var, deps_array, expr
        );
    }

    fn write_var_declaration(out: &mut String, var: &str, value: &Option<String>) {
        let val_expr = if let Some(val) = value {
            util::parse_js_expr(val)
        } else {
            "undefined".to_string()
        };

        let _ = write!(out, "const {} = Observable({});", var, val_expr);
    }

    fn generate_conditional_branch_body(
        html: &str,
        js: &str,
        condition: &str,
        vars: &str,
        bindings: &str,
    ) -> String {
        format!(
            "
    {{
        condition: () => {},
        evaluation: () => {{
            {}
            const frag = HtmlToFragment(`{}`);
            {}
            {}
            return frag;
        }}
    }},
    ",
            condition, vars, html, bindings, js
        )
    }

    fn write_if_statment(out: &mut String, expr: &str, branch_bodies: &str) {
        let _ = write!(
            out,
            "
    const markers_{0} = FindLimitMarkers('{0}', frag);
    const update_{0} = If(markers_{0}, [
        {1}
    ]);
    update_{0}();
    ",
            expr, branch_bodies
        );
    }
    
    fn write_dependecy_binding(out: &mut String, dependency: &str, expr: &str) {
        let _ = write!(out, "{}.addSubscriber(update_{});", dependency, expr);
    }
    
}

impl JSGenerator for minify::Js {
    fn write_js_element_event(
        out_event_binding: &mut String,
        out_event_element_decleration: &mut String,
        events: HtmlEventList,
    ) {
        let _ = write!(
            out_event_element_decleration,
            r#"const elm_{0}=frag.querySelector('[{1}="{0}"]');
            "#,
            events.id, HEML_ID_ATTRIBUTE_KEY,
        );

        for event in events.iter() {
            let _ = write!(
                out_event_binding,
                r#"elm_{}.addEventListener('{}',{}(event)=>{{{}}});"#,
                events.id,
                event.event_name,
                if event.is_async { "async" } else { "" },
                event.event_body
            );
        }
    }

    fn write_global_scope(out: &mut String) {
        let _ = write!(out, "const frag=document;");
    }

    fn write_expr_bind(out: &mut String, var: &str, deps_array: &str, expr: &str) {
        let _ = write!(
            out,
            "BindExpression(FindMarker('{}',frag),{},()=>({}));",
            var, deps_array, expr
        );
    }

    // TODO: Context minifization
    fn write_var_declaration(out: &mut String, var: &str, value: &Option<String>) {
        let val_expr = if let Some(val) = value {
            util::parse_js_expr(val)
        } else {
            "undefined".to_string()
        };

        let _ = write!(out, "const {}=Observable({});", var, val_expr);
    }

    fn generate_conditional_branch_body(
        html: &str,
        js: &str,
        condition: &str,
        vars: &str,
        bindings: &str,
    ) -> String {
        todo!()
    }

    fn write_dependecy_binding(out: &mut String, dependency: &str, expr: &str) {
        todo!()
    }
    
    fn write_if_statment(out: &mut String, expr: &str, branch_bodies: &str) {
        todo!()
    }
}

impl JSGenerator for minify::All {
    fn write_js_element_event(
        out_event_binding: &mut String,
        out_event_element_decleration: &mut String,
        events: HtmlEventList,
    ) {
        minify::Js::write_js_element_event(
            out_event_binding,
            out_event_element_decleration,
            events,
        );
    }

    fn write_global_scope(out: &mut String) {
        minify::Js::write_global_scope(out);
    }

    fn write_expr_bind(out: &mut String, var: &str, deps_array: &str, expr: &str) {
        minify::Js::write_expr_bind(out, var, deps_array, expr);
    }

    fn write_var_declaration(out: &mut String, var: &str, value: &Option<String>) {
        minify::Js::write_var_declaration(out, var, value);
    }

    fn generate_conditional_branch_body(
        html: &str,
        js: &str,
        condition: &str,
        vars: &str,
        bindings: &str,
    ) -> String {
        minify::Js::generate_conditional_branch_body(html, js, condition, vars, bindings)
    }

    
    fn write_dependecy_binding(out: &mut String, dependency: &str, expr: &str) {
        todo!()
    }
    
    fn write_if_statment(out: &mut String, expr: &str, branch_bodies: &str) {
        todo!()
    }
}

