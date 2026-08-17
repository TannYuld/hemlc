function Observable(defaultValue) {
    return new ObservableExpr(defaultValue, []);
}

class ObservableExpr {
    constructor(defaultValue, subscribers) {
        this.subscribers = subscribers;
        this._value = this._wrap(defaultValue);

        Object.defineProperty(this, "value", {
            get: () => this._value,
            set: (newVal) => {
                this._value = this._wrap(newVal);
                this._notify();
            }
        });
    }

    // get value() {
    //     return this._value;
    // }

    // set value(value) {
    //     this._value = this._wrap(value);
    //     this._notify();
    // }

    getValue() {
        return this._value;
    }

    setValue(value) {
        this.value = value;
    }

    addSubscriber(expr) {
        this.subscribers.push(expr);
    }

    _notify() {
        for (const sub of this.subscribers) {
            sub();
        }
    }

    _wrap(value) {
        if (typeof value !== "object" || value === null) {
            return value;
        }

        const self = this;

        return new Proxy(value, {
            get(obj, prop) {
                if (prop === "target") {
                    return obj;
                }
                return obj[prop];
            },
            set(obj, prop, newVal) {
                obj[prop] = newVal;
                if (prop !== "length") {
                    self._notify();
                }
                return true;
            }
        });
    }
}

function FindLimitMarkers(marker, fragment) {
    const walker = document.createTreeWalker(
        fragment === undefined ? document.body : fragment,
        NodeFilter.SHOW_COMMENT,
        {
            acceptNode: (node) => {
                const node_text = node.textContent.trim().slice(7);

                if ((node_text.startsWith("start__") && node_text.slice(7) === marker) ||
                    (node_text.startsWith("end__") && node_text.slice(5) === marker)) {
                    return NodeFilter.FILTER_ACCEPT;
                }

                return NodeFilter.FILTER_REJECT;
            }
        }
    );
    return [walker.nextNode(), walker.nextNode()];
}

function FindMarker(marker, fragment) {
    const walker = document.createTreeWalker(
        fragment === undefined ? document.body : fragment,
        NodeFilter.SHOW_COMMENT,
        {
            acceptNode: (node) => {
                const node_text = node.textContent.trim().slice(7);

                if (node_text === marker) {
                    return NodeFilter.FILTER_ACCEPT;
                }

                return NodeFilter.FILTER_REJECT;
            }
        }
    );
    return walker.nextNode();
}

function ClearBetweenMarkers(markers) {
    let current = markers[0].nextSibling;
    while (current && current !== markers[1]) {
        let next = current.nextSibling;
        current.remove();
        current = next;
    }
}

function BindValue(marker, observable, getter = () => observable.value) {
    const textNode = document.createTextNode(getter());
    marker.after(textNode);

    observable.addSubscriber(() => {
        textNode.textContent = getter();
    });
}

function PlaceBetweenMarkers(markers, node) {
    markers[0].after(typeof node !== "object" ? document.createTextNode(node) : node);
}

function HtmlToFragment(htmlString) {
    const template = document.createElement('template');
    template.innerHTML = htmlString.trim();
    return template.content;
}

function If(markers, conditions) {
    const update = () => {
        ClearBetweenMarkers(markers);
        for (const cond of conditions) {
            if (cond.condition()) {
                const node = cond.evaluation();
                PlaceBetweenMarkers(markers, node);
                break;
            }
        }
    };
    return update;
}

function For(markers, listObservable, keyFn, renderItemFn) {
    let domCache = new Map();

    const update = () => {
        const newList = listObservable.value;
        const newCache = new Map();

        let currentCursor = markers[0];
        for (let i = 0; i < newList.length; i++) {
            const item = newList[i];
            const key = keyFn(item, i);
            let itemMarkers;

            if (domCache.has(key)) {
                itemMarkers = domCache.get(key);

                if (itemMarkers[0].previousSibling !== currentCursor) {
                    const fragment = document.createDocumentFragment();
                    let curr = itemMarkers[0];
                    const end = itemMarkers[1];

                    while (curr !== end) {
                        let next = curr.nextSibling;
                        fragment.appendChild(curr);
                        curr = next;
                    }
                    fragment.appendChild(end);

                    currentCursor.after(fragment);
                }

                domCache.delete(key);
            } else {
                const fragment = document.createDocumentFragment();
                const start = document.createComment("item_start");
                const end = document.createComment("item_end");

                fragment.appendChild(start);
                fragment.appendChild(renderItemFn(item, i));
                fragment.appendChild(end);

                currentCursor.after(fragment);
                itemMarkers = [start, end];
            }

            newCache.set(key, itemMarkers);
            currentCursor = itemMarkers[1];
        }

        domCache.forEach((itemMarkers) => {
            ClearBetweenMarkers(itemMarkers);
            itemMarkers[0].remove();
            itemMarkers[1].remove();
        });

        domCache = newCache;
    };

    listObservable.addSubscriber(update);
    return update;
}
