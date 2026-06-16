# Deploying New Proteus to your students

The student app is the `runtime/` folder — a fully static site (one HTML page
plus the embedded WASM core, ~100 KB total). Hosting it is free and gives every
student a URL that works on any laptop or phone, installs as an app, and runs
offline after the first visit.

## One-time setup (GitHub Pages)

You're comfortable with git, so this is the lightest path: push the repo once,
and a GitHub Action republishes the app automatically on every later push.

1. **Initialize git and push** from the project root (your `picsim` repo already
   exists on GitHub):

   ```sh
   git init
   git add -A
   git commit -m "picsim: cycle-accurate PIC16F628A simulator"
   git branch -M main
   git remote add origin https://github.com/santibianco/picsim.git
   git push -u origin main
   ```

   A bundled `.gitignore` keeps Rust's ~68 MB `core/target/` build cache out of git.

2. **Enable Pages**: repo → **Settings → Pages** → under *Build and deployment*,
   set **Source = GitHub Actions**.

3. The bundled workflow (`.github/workflows/pages.yml`) runs on each push to
   `main`. Watch it under the **Actions** tab; when it goes green your URL is:

   ```
   https://santibianco.github.io/picsim/
   ```

   (Settings → Pages shows the exact link.)

Share that URL with students — done. The workflow publishes `runtime/` **without**
`authoring.html`, so your instructor editor never reaches the public site.

## Updating the app

Anything you change under `runtime/` (a new bundled lab diagram, a UI tweak) goes
live by pushing:

```sh
git add -A && git commit -m "update" && git push
```

If you rebuilt the WASM core, re-embed it first (see `STATUS.md` step 3), then push.

## Embedding in Moodle

Because it's a normal HTTPS URL, the simulator drops straight into a course:

1. In the course, **Turn editing on → Add an activity or resource → Page**
   (or edit any Label / HTML block).
2. In the editor click the **`< >`** (HTML) button and paste:

   ```html
   <iframe src="https://santibianco.github.io/picsim/"
           width="960" height="700" style="max-width:100%;border:0"
           allow="fullscreen"></iframe>
   ```

3. Save. Students use the simulator inline, inside the course page.

Notes:

- It must be **https** (Pages is) — Moodle blocks http iframes.
- If your Moodle locks down the editor and strips iframes, paste the URL as a
  plain link instead — it opens the full app in a new tab.
- **Install / offline works from the direct URL, not from inside the iframe**
  (browsers restrict installation in frames). So point students at the URL for the
  installable, offline-capable app; treat the Moodle iframe as a convenience.

## Install + offline (PWA)

On the direct URL the browser offers **Install** (an icon in the address bar, or
"Add to Home Screen" on phones). Once installed it opens in its own window with an
icon and runs **fully offline** — the whole app is cached on first visit. Good for
students at home with flaky internet, or a lab with none.

## Alternative: your own / university server

The app is plain static files. To use other hosting, copy the contents of
`runtime/` (minus `authoring.html`) to any web server that serves over **https**.
No build step, no backend.
