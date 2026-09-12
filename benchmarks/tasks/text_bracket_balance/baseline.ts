function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function closing(ch: string): string {
    if ((ch == ")")) {
        return "(";
    }
    if ((ch == "]")) {
        return "[";
    }
    return "{";
}

export function solve(text: string): boolean {
    let stack: string = "";
    let pos: number = 0;
    while ((pos < length(text))) {
        let ch: string = at(text, pos);
        if (contains("([{", ch)) {
            stack = (stack + ch);
        }
        else {
            if (contains(")]}", ch)) {
                if ((length(stack) == 0)) {
                    return false;
                }
                if ((at(stack, (length(stack) - 1)) != closing(ch))) {
                    return false;
                }
                stack = part(stack, 0, (length(stack) - 1));
            }
        }
        pos = (pos + 1);
    }
    return (stack == "");
}
