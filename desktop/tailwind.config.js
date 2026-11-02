import frappeUIPreset, { content } from 'frappe-ui/tailwind'

export default {
  presets: [frappeUIPreset],
  content: ['./index.html', './src/**/*.{vue,ts}', ...content],
}
