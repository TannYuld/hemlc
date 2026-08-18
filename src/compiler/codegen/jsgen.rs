use crate::{
    compiler::obfuscation::ObfuscatedExpr,
    core::types::{CodegenStrategy, Compiler},
};

pub trait JsGenerator {
    fn slot_element_replace(&self, name: &ObfuscatedExpr) -> String;

    fn component_initialization(
        &self,
        func_name: &str,
        target_marker: &ObfuscatedExpr,
        js_props: &str,
        component_content: &str,
        variable_zone: &str,
        binding_zone: &str,
    ) -> String;
    fn component_event_handling(&self, id: &str, js_event: &str, logic: &str) -> String;
    fn component_function_declaration(
        &self,
        func_name: &str,
        html_body: &str,
        js_vars: &str,
        js_var_bindings: &str,
        nested_functions: &str,
    ) -> String;

    fn fixed_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String;
    fn reactive_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String;

    fn component_attributes(&self, name: &str) -> String;
}

impl JsGenerator for Compiler {
    fn slot_element_replace(&self, name: &ObfuscatedExpr) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => {
                format!(
    "
    const m_{0} = FindMarker('{0}', frag);
    if (slot_frag) {{
        m_{0}.replaceWith(slot_frag);
    }}else {{
        m_{0}.remove();
    }}
    ",
                    name.0
                )
            }
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => {
                format!(
                    "const m_{0}=FindMarker('{0}',frag);if(slot_frag){{m_{0}.replaceWith(slot_frag);}}else{{m_{0}.remove();}}",
                    name.0
                )
            }
        }
    }

    fn component_initialization(
        &self,
        func_name: &str,
        target_marker: &ObfuscatedExpr,
        js_props: &str,
        slot_content: &str,
        variable_zone: &str,
        binding_zone: &str,
    ) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => {
                format!(
    "
    const target_{0} = FindMarker('{0}', frag);
    const slot_frag_{0} = (() => {{ 
        {1}
        const frag = HtmlToFragment(`{2}`);
        {3}
        return frag;
    }})();
    {4}(target_{0}, {5}, slot_frag_{0});
    ",
                    target_marker.0, variable_zone, slot_content, binding_zone, func_name, js_props
                )
            }
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => {
                format!(
                    "const target_{0}=FindMarker('{0}',frag);const slot_frag_{0}=(()=>{{{1}const frag=HtmlToFragment(`{2}`);{3}return frag;}})();{4}(target_{0},{5},slot_frag_{0});",
                    target_marker.0, variable_zone, slot_content, binding_zone, func_name, js_props
                )
            }
        }
    }

    fn fixed_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
    "
    const marker__{0} = FindMarker(\"{0}\", frag);
    marker__{0}.after(document.createTextNode({1}.value{2}));
    ",
                var_name.0, base_var, remainder
            ),
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => format!(
                "const marker__{0}=FindMarker(\"{0}\",frag);marker__{0}.after(document.createTextNode({1}.value{2}));",
                var_name.0, base_var, remainder
            ),
        }
    }

    fn reactive_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
    "
    const marker__{0} = FindMarker(\"{0}\", frag);
    BindValue(marker__{0}, {1}, () => ({1}.value{2}));
    ",
                var_name.0, base_var, remainder
            ),
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => format!(
                "const marker__{0}=FindMarker(\"{0}\",frag);BindValue(marker__{0},{1},()=>({1}.value{2}));",
                var_name.0, base_var, remainder
            ),
        }
    }

    fn component_event_handling(&self, id: &str, js_event: &str, logic: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
    "
    frag.querySelector('[data-heml-id=\"{}\"]').addEventListener('{}', async (event) => {{
        {}
    }});
    ",
                id, js_event, logic
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
                "frag.querySelector('[data-heml-id=\"{}\"]').addEventListener('{}',async (event)=>{{{}}});",
                id, js_event, logic
            ),
        }
    }

    fn component_function_declaration(
        &self,
        func_name: &str,
        html_body: &str,
        js_vars: &str,
        js_var_bindings: &str,
        nested_functions: &str,
    ) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
    "
    {}
    function {}(marker_target, props, slot_frag) {{
        {}
        const frag = HtmlToFragment(`{}`);
        {}
        marker_target.after(frag);
    }}
    ",
                nested_functions, func_name, js_vars, html_body, js_var_bindings
            ),
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => format!(
                "{}function {}(marker_target,props,slot_frag){{{}const frag=HtmlToFragment(`{}`);{}marker_target.after(frag);}}",
                nested_functions, func_name, js_vars, html_body, js_var_bindings
            ),
        }
    }

    fn component_attributes(&self, name: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "const {0} = (props.{0} !== undefined && props.{0}.addSubscriber) ? props.{0} : Observable(props.{0});",
                name
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
                "const {0}=(props.{0}!==undefined&&props.{0}.addSubscriber)?props.{0}:Observable(props.{0});",
                name
            ),
        }
    }
}
