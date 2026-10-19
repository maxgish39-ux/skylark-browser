# Skylark

<img src="docs/brand/skylark-icon-512.png" width="96" align="right" alt="Skylark logo"/>

**Skylark** is a clean, fast browser for Android. Pages are drawn by the system WebView (Chromium), so JavaScript, video, modern CSS, cookies and logins work the way sites are meant to look.

## What's in 0.6.0

- **Skylark AI (Groq).** Chat assistant and one-tap **Summarize page**. Uses the Groq OpenAI-compatible API. Model picker (Llama 3.1 8B, Llama 3.3 70B, Mixtral, Gemma 2). API key editable in Settings (pre-filled for this build — **rotate the key**; it can be extracted from the APK).
- **Reader mode.** Simplifies the current page (hides nav/ads, widens text).
- **Real websites.** JavaScript, DOM storage, HTML5 video/audio (fullscreen), cookies, file upload, popups, `mailto:`/`tel:`.
- **Interface.** Address pill, progress bar, bottom bar, light/dark themes.
- **Tabs, start page, star bookmarks.**
- **Full Settings.** Search engines, theme, homepage, JS/cookies/popups/DNT, clear data, downloads, **Skylark AI**, account, about.
- **Local accounts.** Sign up / sign in / profile / logout / delete. **Continue with Skylark** after sign-up. Device-only storage.
- **History** (last 80) and **Bookmarks** manager.
- **Find in page.**

## Security note on the AI key

The Groq API key lives in SharedPreferences and is shipped as a default in this source tree. **Anyone who installs the APK can extract it.** Use a dedicated Groq key you can revoke, or clear the field and paste your own in Settings → Skylark AI.

## Not yet

Saved site passwords, ad blocking, camera/mic/location prompts, true incognito, tab previews, cloud sync, streaming token UI.

## Build

JDK 17, Android SDK 35:

```sh
./gradlew assembleDebug
# app/build/outputs/apk/debug/app-debug.apk
```

Or GitHub Actions → Build Skylark APK.

## Legacy engine

`legacy-engine/` is kept for reference only; the app uses system WebView.
