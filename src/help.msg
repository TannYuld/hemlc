hemlc -- the Hyperextended Markup Language compiler

USAGE:
    hemlc <input>... [OPTIONS]

INPUTS
    page.heml              compile one page
    a.heml b.heml          compile several pages
    ./site/                compile every .heml page under a directory,
                           recursively

OPTIONS
    -o, --out <path>       output file for a single input, or output directory
                           for several (default: alongside each input)
    -w, --watch            rebuild whenever a page or one of its components
                           changes; Ctrl-C to stop
    -m  --minify <level>   choose how much to minify output html. (Default 2)
                           0 = Minify None
                           1 = Minify Js Only
                           2 = Minify All
        --check            parse and validate only, write nothing
    -q, --quiet            only report errors
    -V, --version          show version
    -h, --help             show this message

The runtime is always inlined, so every page is a single self-contained file.
