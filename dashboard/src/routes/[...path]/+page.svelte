<script lang="ts">
  // The dashboard's one page (T-428), at any path: `atm serve` serves it at `/`, the e2e fixture as
  // `/tasks-dashboard.html`. Its views still live in the hash (#project=…, #view=…) until T-429 gives them routes.
  import '../../app.css';
  import '../../themes/default.css';
  import '../../themes/nord.css';
  import '../../themes/solarized.css';
  import '../../themes/catppuccin.css';
  import '../../themes/tokyo-night.css';
  import '../../themes/ayu.css';
  import '../../themes/night-owl.css';
  import '../../themes/gruvbox.css';
  import '../../themes/noctis.css';
  import '../../themes/sublime-material.css';
  import '../../themes/high-contrast.css';
  import '../../themes/sticky-notes.css';
  import '../../themes/hand-drawn.css';
  import '../../themes/doc-styles.css';
  import App from '../../App.svelte';
</script>

<App />
