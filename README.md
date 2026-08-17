# HEMLC - Hyperextended Markup Language Compiler

A Compiler for HEML, a superset of HTML which compiles down to plain HTML with tiny js core of ~2kb and respective reactivity.

## *Why make such tool?*
<br/>
Because I always wanted some basic reactivity functionality in plain HTML, I made this "quick and dirty" tool to make my life easier.

This tool isn't a full-fledge tool, this is a little mere compiler for a tiny superset of HTML which I carved for.

**There are still some features and tags which isn't implemented fully yet, therefore I added a self update feature for ease of use.**

HEML language and this compiler isn't built with best design practices in mind. I made this tool purely for myself so I designed it purely for my personal choices.

## *How does it work?*
heml component file (optional) --> heml file --> hemlc --> html + inline 2kb js core + respective reactive js functionality

A valid html is also valid for hemlc without any heml tags or custom imported tags.

## *What does HEML contains?*
<br/>
Further examples of all these new `HEML` tags can be found in examples.
There are some reactive value based observable variables:

- `<var>` - This tag generates block scoped observable variables.
- `<value>` - This tag subscribes to a observable variable and render them reactivly. So that when observable value is changed, it rerenderd again automaticly.
- `<data>` - For giving a complex object, a proper context so that plain html elements can be used to represent that complex data.
- `<key>` - Specifing some detials in `<data>` block.

It has a couple of control block tags such as these, which lets you control the rendering reactivly:
- `<if>` / `<else>` / `<elseif>` - This is used for the most basic reactive conditional rendering.
- `<for>` - Iterative rendering of the block elements. 
- `<match>` / `<arm>` - This is a pattern matching block with rust inspired syntax.

HEML also have isolated component system which can be imported to any heml file. There are also some tags for that purpose.
- `<component>` - This is the body of an component.
- `<attribute>` - Specifies which attributes can be passed to an attribute.
- `<properties>` - Head section of an component
- `<children>` - Pass the given children of an component inside the component self.

## *Examples*

#### Basic Reactivity
```html
<!-- heml file -->
<var name="myVar" value="{5}" />
Static value: <value name="{myVar}" fixed />
<br />
Reactive value: <value name="{myVar}" />
<br />
<button onclick="myVar.value += 1;">+1</button>
```
`value` attribute is optional, thus if it isn't given variable start as js `undefined` to its life.

As you can see above in the example, to edit the value of observable object you need to use `value` field.

If `static` attribute is passed to any `<value>` tag, only pass the given value once at the exact moment of render, there is no reactiviy by observing.

```html
<!-- heml file -->
<var name="person" value="{{title:'Mr', name:'Hyde'}}" />
Hello world, this is <value name="{person.title}" />. <value name="{person.name}" />!
<br />
<button onclick="
    person.value.title = person.value.title === 'Dr' ? 'Mr' : 'Dr';
    person.value.name = person.value.name === 'Jekyll' ? 'Hyde' : 'Jekyll';
">
    Change Persona
</button>
```
Js objects (or array's) can also be used as observable variable. To change their fields you don't need to use `value` field specifically.

#### Component System
You can import other heml components into one main heml file. Then start to use them in the file you import them.

This is how a component file and heml doc which uses that looks like:
```html
<!--component.heml-->
<!DOCTYPE component>
<properties>
    <attribute name="val"/>
    <attribute name="optionalVal" optional/>
</properties>
<component>
    The value is: <value name="{val}" />
    The optionalValue is: <value name="{optionalVal}" />
    <button onclick="val.value += 1;">+1</button>
</component>


<!--index.heml-->
<!DOCTYPE heml>
<html>
    <head>
        <import src="./component.heml" as="comp" />
    </head>
    <body>
        Below are the two distinct, isolated block-scoped component instances.
        <comp val="{3}" optionalVal="Hey!" />
        <comp val="{5}" />
    </body>
</html>
```

A non-recursive component chaining can be achived.
Any passed childeren can be passed via `<children/>` tag in an component.

```html
<!--foo.heml-->
<!DOCTYPE component>
<import src="./bar.heml" as="Bar" />
<properties>
</properties>
<component>
    Hello, <children/>.
    <br/>
    This is component FOO.
    <br/>
    <Bar/>
</component>


<!--bar.heml-->
<!DOCTYPE component>
<properties>
</properties>
<component>
    This is component BAR! 
</component>


<!--index.heml-->
<!DOCTYPE heml>
<html>
    <head>
        <import src="./foo.heml" />
    </head>
    <body>
        <foo>World</foo>
    </body>
</html>
```

Field `as` is used to customize the name which will be used across the document and it is optional. If not specified, file name will be selected for the custom tag.

---

I didn't implemented other features and their tags. So I will include them here after I finish them.

#### Compiler usage
Rust project can be build with this command.
```bash
cargo build --release
```

After building and placing it into its definitive location, you can start to use it.
```bash
hemlc page.heml                    # -> page.html
hemlc index.heml about.heml        # several pages
hemlc ./site/ --out ./dist         # every page under a directory (except components)
hemlc ./site/ --watch
```

Component files doesn't need to be compiled, they will be automaticly resolved during the compilation phase of any heml doc which imports them.

```bash
hemlc --update
```
Can be used to update the compiler.

#### Roadmap
- Conditional rendering blocks `<if>`, `<else>`, `<elseif>`.
- Iterative rendering with `<for>` block.
- Pattern matching with `<match>` and `<arm>` tags.
- Context control with `<data>` and `<key>`.
- VSCode linter and codesuggetsions.
- Better compiler error handling.
