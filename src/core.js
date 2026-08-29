function Observable(defaultValue) {
    return new ObservableExpr(defaultValue, []);
}

class ObservableExpr {
    constructor(defaultValue, subscribers) {
        this.subscribers = subscribers;
        this._value = this._wrap(defaultValue);
        this._updateQueued = false;

        Object.defineProperty(this, "value", {
            get: () => this._value,
            set: (newVal) => {
                this._value = this._wrap(newVal);
                this._notify();
            }
        });
    }

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
        if (this._updateQueued) return;
        this._updateQueued = true;

        queueMicrotask(() => {
            this._updateQueued = false;
            this.subscribers = this.subscribers.filter(sub => sub() !== false);
        });
    }

    _wrap(value) {
        if (typeof value !== "object" || value === null) {
            return value;
        }

        const self = this;

        return new Proxy(value, {
            get(obj, prop) {
                if (prop === 'target') return obj;
                const val = obj[prop];
                return typeof val === 'function' ? val.bind(obj) : val;
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
        if (!marker.isConnected && !marker.parentNode) return false;
        textNode.textContent = getter();
        return true;
    });
}

function BindExpression(marker, observables, getter) {
    const evaluate = () => {
        const val = getter();
        return typeof val === 'function' ? val() : val;
    };

    const textNode = document.createTextNode(String(evaluate()));
    marker.after(textNode);

    if (observables && observables.length > 0) {
        const update = () => {
            if (!marker.isConnected && !marker.parentNode) return false;
            textNode.textContent = String(evaluate());
            return true;
        };

        for (const obs of observables) {
            if (obs && typeof obs.addSubscriber === 'function') {
                obs.addSubscriber(update);
            }
        }
    }
}

function PlaceBetweenMarkers(markers, node) {
    markers[0].after(typeof node !== "object" ? document.createTextNode(node) : node);
}

function HtmlToFragment(htmlString) {
    const template = document.createElement('template');
    template.innerHTML = htmlString.trim();
    return template.content;
}

function isMatch(val, pattern) {
    if (val === pattern) return true;

    if (Array.isArray(pattern) && Array.isArray(val)) {
        if (pattern.length !== val.length) return false;
        return pattern.every((p, i) => isMatch(val[i], p));
    }

    if (typeof pattern === 'object' && pattern !== null && val !== null) {
        return Object.keys(pattern).every(key => isMatch(val[key], pattern[key]));
    }

    return false;
}

function If(markers, conditions) {
    let activeBranchIndex = -1;

    const update = () => {
        if (!markers[0].isConnected && !markers[0].parentNode) return false;

        let matchedIndex = -1;
        for (let i = 0; i < conditions.length; i++) {
            if (conditions[i].condition()) {
                matchedIndex = i;
                break;
            }
        }

        if (matchedIndex !== activeBranchIndex) {
            ClearBetweenMarkers(markers);

            if (matchedIndex !== -1) {
                const node = conditions[matchedIndex].evaluation();
                PlaceBetweenMarkers(markers, node);
            }

            activeBranchIndex = matchedIndex;
        }

        return true;
    };

    return update;
}

function For(markers, listObservable, keyFn, renderItemFn) {
    let domCache = new Map();

    const update = () => {
        if (!markers[0].isConnected && !markers[0].parentNode) return false;

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
        return true;
    };

    listObservable.addSubscriber(update);
    return update;
}
