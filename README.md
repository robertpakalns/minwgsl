# minwgsl

A WGSL minifier, built in Rust and distributed as an NPM package

## Usage
Currently, `minwgsl` has only one export:

```js
export function minify(source: string): string;
```

To use it:

```bash
bun add minwgsl -D
````

```js
import { minify } from "minwgsl";

const source = `@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(pos, 1.0);
}`;

const minified = minify(source);

console.log(minified);

```

Approximate output: 
```
@vertex fn vs_main(@location(0)b:vec3<f32>)->@builtin(position)vec4<f32>{return vec4<f32>(b,1f);}
```

## Credits
* https://github.com/LucentFlux/wgsl-minifier
* https://github.com/gfx-rs/wgpu
