function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function parent_path(text: string): string {
    let pos: number = (length(text) - 1);
    while ((pos >= 0)) {
        if ((at(text, pos) == "/")) {
            return part(text, 0, pos);
        }
        pos = (pos - 1);
    }
    return "";
}

export function add_segment(path: string, segment: string): string {
    if (((segment == "") || (segment == "."))) {
        return path;
    }
    if ((segment == "..")) {
        return parent_path(path);
    }
    return ((path + "/") + segment);
}

export function solve(text: string): string {
    let path: string = "";
    let start: number = 1;
    let pos: number = 1;
    while ((pos <= length(text))) {
        if (((pos == length(text)) || (at(text, pos) == "/"))) {
            path = add_segment(path, part(text, start, pos));
            start = (pos + 1);
        }
        pos = (pos + 1);
    }
    if ((path == "")) {
        return "/";
    }
    return path;
}
