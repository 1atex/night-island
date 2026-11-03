const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');

async function main() {
    const root = __dirname;
    const browser = await chromium.launch({ channel: 'chrome', headless: true });
    const animations = {};
    const names = (await fs.readdir(path.join(root, 'src'))).filter(name => name.endsWith('.svg')).sort();
    await fs.mkdir(path.join(root, 'native'), { recursive: true });
    try {
        const page = await browser.newPage({ reducedMotion: 'no-preference' });
        for (const name of names) {
            const source = await fs.readFile(path.join(root, 'src', name), 'utf8');
            await page.setContent(source);
            const prepared = await page.evaluate(() => {
                const root = document.querySelector('svg');
                const tracks = [];
                const elements = [...root.querySelectorAll('*')];
                elements.forEach((element, i) => {
                    if (!element.id) element.id = `element-${i}`;
                });
                const numbers = value => (String(value).match(/-?\d*\.?\d+(?:e[-+]?\d+)?/gi) || []).map(Number);
                const curve = value => ({ linear: [0, 0, 1, 1], 'ease-in-out': [.42, 0, .58, 1], ease: [.25, .1, .25, 1] }[value] || numbers(value));
                const transform = value => {
                    const result = [0, 0, 0];
                    for (const part of value.matchAll(/(translateX|translateY|translate|rotate)\(([^)]+)\)/g)) {
                        const values = numbers(part[2]);
                        if (part[1] === 'translateX') result[0] = values[0];
                        if (part[1] === 'translateY') result[1] = values[0];
                        if (part[1] === 'translate') [result[0], result[1]] = [values[0], values[1] || 0];
                        if (part[1] === 'rotate') result[2] = values[0];
                    }
                    return result;
                };
                const animations = root.getAnimations({ subtree: true });
                for (const animation of animations) {
                    const element = animation.effect.target;
                    const timing = animation.effect.getTiming();
                    const keyframes = animation.effect.getKeyframes();
                    const origin = numbers(getComputedStyle(element).transformOrigin).slice(0, 2);
                    for (const [property, kind] of [['d', 'path'], ['opacity', 'opacity'], ['transform', 'transform'], ['strokeDashoffset', 'dashOffset'], ['filter', 'color']]) {
                        const frames = keyframes.filter(frame => property in frame);
                        if (!frames.length) continue;
                        if (property === 'filter') {
                            const fill = element.getAttribute('fill');
                            const gradient = root.querySelector(fill.replace('url(', '').replace(')', ''));
                            for (const stop of gradient.querySelectorAll('stop')) {
                                const hex = stop.getAttribute('stop-color').slice(1);
                                const color = [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255);
                                tracks.push({ id: stop.id, kind, duration: timing.duration / 1000, delay: timing.delay / 1000, origin: [0, 0], frames: frames.map(frame => ({ at: frame.computedOffset, value: [...color.map(c => Math.min(1, c * numbers(frame.filter)[0])), 1], easing: curve(frame.easing) })) });
                            }
                            continue;
                        }
                        const lengthScale = property === 'strokeDashoffset' && element.hasAttribute('pathLength') ? element.getTotalLength() / Number(element.getAttribute('pathLength')) : 1;
                        const values = frames.map(frame => ({ at: frame.computedOffset, value: property === 'transform' ? transform(frame[property]) : numbers(frame[property]).map(n => n * lengthScale), easing: curve(frame.easing) }));
                        if (values.some(frame => frame.easing.length !== 4 || frame.value.length !== values[0].value.length)) throw Error(`Incompatible animation: ${element.id} ${property}`);
                        tracks.push({ id: element.id, kind, duration: timing.duration / 1000, delay: timing.delay / 1000, origin, frames: values });
                        if (property === 'd') element.setAttribute('d', frames[0].d.match(/^path\(["'](.*)["']\)$/)[1]);
                        if (property === 'strokeDashoffset') {
                            element.setAttribute('stroke-dashoffset', String(values[0].value[0]));
                            element.setAttribute('stroke-dasharray', numbers(getComputedStyle(element).strokeDasharray).map(n => n * lengthScale).join(' '));
                        }
                    }
                }
                root.querySelectorAll('style,title,desc').forEach(element => element.remove());
                for (const element of [root, ...root.querySelectorAll('*')]) {
                    for (const attr of ['style', 'class', 'role', 'aria-labelledby', 'pathLength']) element.removeAttribute(attr);
                }
                root.setAttribute('width', '24');
                root.setAttribute('height', '24');
                return { svg: new XMLSerializer().serializeToString(root), tracks };
            });
            await fs.writeFile(path.join(root, 'native', name), `${prepared.svg}\n`);
            animations[name.slice(0, -4)] = prepared.tracks;
        }
    } finally {
        await browser.close();
    }
    await fs.writeFile(path.join(root, 'animation.json'), `${JSON.stringify(animations)}\n`);
    const entries = names.map(name => `    ("${name.slice(0, -4)}", include_bytes!("../../../assets/weather/native/${name}")),`).join('\n');
    await fs.writeFile(path.join(root, '../../src/ui/weather_icons/assets.rs'), `pub const ASSETS: &[(&str, &[u8])] = &[\n${entries}\n];\n`);
    process.stdout.write(`Prepared ${names.length} native SVGs and their animation tracks.\n`);
}

main().catch(error => {
    console.error(error);
    process.exitCode = 1;
});
