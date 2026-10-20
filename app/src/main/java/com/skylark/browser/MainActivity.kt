package com.skylark.browser

import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.app.DownloadManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.ColorStateList
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.text.InputType
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.webkit.CookieManager
import android.webkit.RenderProcessGoneDetail
import android.webkit.URLUtil
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject
import java.net.URLEncoder
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.min

/**
 * Skylark 0.6 — clean Android browser with Skylark AI powered by system WebView.
 * Address pill, tabs, start page, full Settings, local accounts, history, bookmarks.
 */
class MainActivity : Activity() {
    companion object {
        const val HOME = "skylark://home"
        private const val PREFS = "skylark"
        private const val REQ_FILE = 41
        private val ENGINES = listOf(
            "Google" to "https://www.google.com/search?q=",
            "DuckDuckGo" to "https://duckduckgo.com/?q=",
            "Bing" to "https://www.bing.com/search?q=",
            "Brave" to "https://search.brave.com/search?q=",
            "Startpage" to "https://www.startpage.com/sp/search?query=",
            "Ecosia" to "https://www.ecosia.org/search?q="
        )
        private val TILE_COLORS = intArrayOf(
            0xFF4F46E5.toInt(), 0xFF0EA5E9.toInt(), 0xFF10B981.toInt(),
            0xFFF59E0B.toInt(), 0xFFEF4444.toInt(), 0xFF8B5CF6.toInt()
        )
        private const val MAX_HISTORY = 80
    }

    private class Shortcut(val title: String, val url: String, val color: Int, val custom: Boolean)

    private class Tab {
        var web: WebView? = null
        var url: String = HOME
        var title: String = ""
        var progress: Int = 100
        var error: String? = null
        var showHome: Boolean = true
    }

    private class HistoryEntry(val title: String, val url: String, val ts: Long)

    private val tabs = mutableListOf<Tab>()
    private var curIndex = 0
    private val cur: Tab get() = tabs[curIndex]
    private var desktopMode = false
    private lateinit var accounts: AccountManager
    private lateinit var ai: AiHelper

    private var customView: View? = null
    private var customCallback: WebChromeClient.CustomViewCallback? = null
    private var fileCallback: ValueCallback<Array<Uri>>? = null

    private lateinit var root: LinearLayout
    private lateinit var contentHost: FrameLayout
    private lateinit var address: EditText
    private lateinit var lockIcon: ImageView
    private lateinit var reloadButton: ImageButton
    private lateinit var progress: ProgressBar
    private lateinit var homeScroll: ScrollView
    private lateinit var shortcutsBox: LinearLayout
    private lateinit var webHost: FrameLayout
    private lateinit var errorBox: LinearLayout
    private lateinit var errorDetail: TextView
    private lateinit var backButton: ImageButton
    private lateinit var forwardButton: ImageButton
    private lateinit var starButton: ImageButton
    private lateinit var tabCount: TextView
    private lateinit var overlay: FrameLayout

    private fun color(id: Int): Int = getColor(id)
    private fun prefs() = getSharedPreferences(PREFS, MODE_PRIVATE)

    // ---------------------------------------------------------------- lifecycle

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        accounts = AccountManager(this)
        ai = AiHelper(this)
        // Seed default API key once if empty
        if (prefs().getString("ai_api_key", null).isNullOrBlank()) {
            prefs().edit().putString("ai_api_key", AiHelper.DEFAULT_KEY).apply()
        }
        applyForcedTheme()
        desktopMode = prefs().getBoolean("desktop_default", false)
        buildUi()
        setContentView(root)
        if (tabs.isEmpty()) {
            tabs.add(Tab())
            curIndex = 0
        }
        refresh()
        handleIntent(intent)
        // First-run: gently invite account if none
        if (!prefs().getBoolean("seen_account_hint", false) && !accounts.isLoggedIn()) {
            prefs().edit().putBoolean("seen_account_hint", true).apply()
        }
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        val data = intent?.data?.toString() ?: return
        if (!(data.startsWith("http://") || data.startsWith("https://"))) return
        if (cur.showHome && cur.web == null) navigate(data) else openInNewTab(data)
    }

    override fun onPause() {
        cur.web?.onPause()
        CookieManager.getInstance().flush()
        super.onPause()
    }

    override fun onResume() {
        super.onResume()
        cur.web?.onResume()
    }

    override fun onDestroy() {
        for (t in tabs) t.web?.destroy()
        super.onDestroy()
    }

    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        when {
            overlay.childCount > 0 -> closeOverlay()
            customView != null -> hideCustomView()
            address.hasFocus() -> releaseAddressFocus()
            cur.showHome && cur.web == null -> if (tabs.size > 1) closeTab(curIndex) else super.onBackPressed()
            cur.showHome -> goBack()
            cur.error != null || cur.web?.canGoBack() == true -> goBack()
            tabs.size > 1 -> closeTab(curIndex)
            else -> { cur.showHome = true; refresh() }
        }
    }

    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode == REQ_FILE) {
            fileCallback?.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(resultCode, data))
            fileCallback = null
        } else super.onActivityResult(requestCode, resultCode, data)
    }

    // ---------------------------------------------------------------- theme

    private fun applyForcedTheme() {
        val mode = prefs().getString("theme_mode", "system") ?: "system"
        if (Build.VERSION.SDK_INT >= 29) {
            when (mode) {
                "light" -> resources.configuration.uiMode =
                    (resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK.inv()) or
                        android.content.res.Configuration.UI_MODE_NIGHT_NO
                "dark" -> resources.configuration.uiMode =
                    (resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK.inv()) or
                        android.content.res.Configuration.UI_MODE_NIGHT_YES
            }
        }
    }

    // ---------------------------------------------------------------- UI shell

    @Suppress("DEPRECATION")
    private fun buildUi() {
        root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
            isFocusable = true
            isFocusableInTouchMode = true
        }
        root.setOnApplyWindowInsetsListener { v, insets ->
            if (Build.VERSION.SDK_INT >= 30) {
                val b = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.ime())
                v.setPadding(b.left, b.top, b.right, b.bottom)
            } else {
                v.setPadding(insets.systemWindowInsetLeft, insets.systemWindowInsetTop, insets.systemWindowInsetRight, insets.systemWindowInsetBottom)
            }
            insets
        }

        // Top: address pill + menu
        val topBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(12), dp(8), dp(6), dp(8))
        }
        val pill = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            background = rounded(color(R.color.sky_pill), 26)
        }
        lockIcon = ImageView(this).apply { setImageResource(R.drawable.ic_search) }
        pill.addView(lockIcon, LinearLayout.LayoutParams(dp(18), dp(18)).apply { leftMargin = dp(14) })
        address = EditText(this).apply {
            background = null
            hint = "Search or type a URL"
            textSize = 15f
            setTextColor(color(R.color.sky_ink))
            setHintTextColor(color(R.color.sky_muted))
            maxLines = 1
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            imeOptions = EditorInfo.IME_ACTION_GO
            setSelectAllOnFocus(true)
            setPadding(dp(10), 0, dp(4), 0)
            setOnEditorActionListener { _, actionId, _ ->
                if (actionId == EditorInfo.IME_ACTION_GO || actionId == EditorInfo.IME_ACTION_DONE) { submitAddress(); true } else false
            }
            setOnFocusChangeListener { _, focused ->
                if (focused) {
                    setText(if (cur.showHome) "" else currentUrl())
                    post { selectAll() }
                } else {
                    setText(displayFor(if (cur.showHome) HOME else currentUrl()))
                }
            }
        }
        pill.addView(address, LinearLayout.LayoutParams(0, dp(46), 1f))
        reloadButton = iconButton(R.drawable.ic_refresh, "Reload") { onReloadOrStop() }
        pill.addView(reloadButton, LinearLayout.LayoutParams(dp(42), dp(46)))
        topBar.addView(pill, LinearLayout.LayoutParams(0, dp(46), 1f))
        val moreButton = iconButton(R.drawable.ic_more, "Menu") { }
        moreButton.setOnClickListener { showMenu(moreButton) }
        topBar.addView(moreButton, LinearLayout.LayoutParams(dp(44), dp(46)).apply { leftMargin = dp(2) })
        root.addView(topBar)

        progress = ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 100
            progressTintList = ColorStateList.valueOf(color(R.color.sky_accent))
            progressBackgroundTintList = ColorStateList.valueOf(Color.TRANSPARENT)
            visibility = View.INVISIBLE
            minimumHeight = dp(2)
        }
        root.addView(progress, LinearLayout.LayoutParams(-1, dp(3)))

        // Content host (home / web / error)
        contentHost = FrameLayout(this)
        homeScroll = ScrollView(this).apply {
            isFillViewport = true
            addView(buildHome())
        }
        contentHost.addView(homeScroll, FrameLayout.LayoutParams(-1, -1))

        webHost = FrameLayout(this)
        contentHost.addView(webHost, FrameLayout.LayoutParams(-1, -1))
        webHost.visibility = View.GONE

        errorBox = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(dp(32), dp(48), dp(32), dp(32))
            visibility = View.GONE
        }
        errorBox.addView(TextView(this).apply {
            text = "Couldn't load page"
            textSize = 20f
            setTextColor(color(R.color.sky_ink))
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            gravity = Gravity.CENTER
        })
        errorDetail = TextView(this).apply {
            textSize = 14f
            setTextColor(color(R.color.sky_muted))
            gravity = Gravity.CENTER
            setPadding(0, dp(12), 0, dp(24))
        }
        errorBox.addView(errorDetail)
        val retry = TextView(this).apply {
            text = "Try again"
            textSize = 15f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            background = rounded(color(R.color.sky_accent), 12)
            setPadding(dp(28), dp(12), dp(28), dp(12))
            setOnClickListener { onReloadOrStop() }
        }
        errorBox.addView(retry, LinearLayout.LayoutParams(-2, -2))
        contentHost.addView(errorBox, FrameLayout.LayoutParams(-1, -1))

        root.addView(contentHost, LinearLayout.LayoutParams(-1, 0, 1f))

        // Bottom bar
        val bottom = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(4), dp(4), dp(4), dp(6))
            setBackgroundColor(color(R.color.sky_bg))
        }
        backButton = iconButton(R.drawable.ic_back, "Back") { goBack() }
        forwardButton = iconButton(R.drawable.ic_forward, "Forward") { goForward() }
        val homeBtn = iconButton(R.drawable.ic_home, "Home") {
            cur.showHome = true
            cur.error = null
            refresh()
        }
        starButton = iconButton(R.drawable.ic_star, "Bookmark") { toggleBookmark() }
        val tabsBtn = FrameLayout(this)
        val tabsIcon = iconButton(R.drawable.ic_more, "Tabs") { showTabs() }
        // Re-use more icon as placeholder; label shows count
        tabCount = TextView(this).apply {
            text = "1"
            textSize = 12f
            setTextColor(color(R.color.sky_ink))
            gravity = Gravity.CENTER
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        }
        tabsBtn.addView(tabsIcon, FrameLayout.LayoutParams(-1, -1))
        tabsBtn.addView(tabCount, FrameLayout.LayoutParams(-1, -1))
        tabsBtn.setOnClickListener { showTabs() }

        listOf(backButton, forwardButton, homeBtn, starButton).forEach {
            bottom.addView(it, LinearLayout.LayoutParams(0, dp(48), 1f))
        }
        bottom.addView(tabsBtn, LinearLayout.LayoutParams(0, dp(48), 1f))
        root.addView(bottom)

        // Full-screen overlay for Settings / Account / History / Auth
        overlay = FrameLayout(this).apply {
            setBackgroundColor(color(R.color.sky_bg))
            visibility = View.GONE
            elevation = 24f
        }
        root.addView(overlay, LinearLayout.LayoutParams(-1, -1))
    }

    private fun buildHome(): View {
        val box = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(dp(20), dp(36), dp(20), dp(24))
        }
        val logo = ImageView(this).apply {
            setImageResource(R.drawable.ic_logo_mark)
            scaleType = ImageView.ScaleType.FIT_CENTER
        }
        box.addView(logo, LinearLayout.LayoutParams(dp(72), dp(72)).apply { bottomMargin = dp(8) })
        box.addView(TextView(this).apply {
            text = "Skylark"
            textSize = 28f
            setTextColor(color(R.color.sky_ink))
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            gravity = Gravity.CENTER
        })
        // Account chip on start page
        val user = accounts.currentUser()
        if (user != null) {
            box.addView(TextView(this).apply {
                text = "Hi, ${user.displayName}"
                textSize = 13f
                setTextColor(color(R.color.sky_muted))
                gravity = Gravity.CENTER
                setPadding(0, dp(4), 0, 0)
                setOnClickListener { showAccountScreen() }
            })
        } else {
            box.addView(TextView(this).apply {
                text = "Sign in for a personal start page"
                textSize = 13f
                setTextColor(color(R.color.sky_accent))
                gravity = Gravity.CENTER
                setPadding(0, dp(4), 0, 0)
                setOnClickListener { showLoginScreen() }
            })
        }
        box.addView(View(this), LinearLayout.LayoutParams(-1, dp(20)))
        shortcutsBox = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        box.addView(shortcutsBox, LinearLayout.LayoutParams(-1, -2))
        refreshShortcuts()
        return box
    }

    private fun refreshShortcuts() {
        shortcutsBox.removeAllViews()
        val all = defaultShortcuts() + loadCustomShortcuts()
        val rows = all.chunked(4)
        for (rowItems in rows) {
            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
            for (s in rowItems) row.addView(tile(s), LinearLayout.LayoutParams(0, -2, 1f))
            repeat(4 - rowItems.size) { row.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f)) }
            shortcutsBox.addView(row, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(14) })
        }
    }

    private fun tile(s: Shortcut): View {
        val box = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(dp(2), dp(4), dp(2), dp(4))
            setOnClickListener { navigate(s.url) }
            if (s.custom) setOnLongClickListener {
                AlertDialog.Builder(this@MainActivity).setTitle("Remove from start page?").setMessage(s.title)
                    .setPositiveButton("Remove") { _, _ -> removeCustomShortcut(s.url) }
                    .setNegativeButton("Cancel", null).show()
                true
            }
        }
        val letter = (s.title.trim().firstOrNull() ?: '?').uppercaseChar().toString()
        val square = TextView(this).apply {
            text = letter; textSize = 22f; setTextColor(Color.WHITE); gravity = Gravity.CENTER
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            background = rounded(s.color, 18)
        }
        box.addView(square, LinearLayout.LayoutParams(dp(56), dp(56)))
        box.addView(TextView(this).apply {
            text = s.title; textSize = 12f; setTextColor(color(R.color.sky_ink)); gravity = Gravity.CENTER
            maxLines = 1; ellipsize = TextUtils.TruncateAt.END
            setPadding(0, dp(7), 0, 0)
        }, LinearLayout.LayoutParams(-1, -2))
        return box
    }

    private fun iconButton(res: Int, description: String, action: () -> Unit): ImageButton {
        val tv = TypedValue()
        theme.resolveAttribute(android.R.attr.selectableItemBackgroundBorderless, tv, true)
        return ImageButton(this).apply {
            setImageResource(res)
            contentDescription = description
            setBackgroundResource(tv.resourceId)
            setOnClickListener { action() }
            imageTintList = ColorStateList.valueOf(color(R.color.sky_ink))
        }
    }

    private fun rounded(fill: Int, radiusDp: Int, stroke: Int? = null, strokeDp: Int = 1) = GradientDrawable().apply {
        shape = GradientDrawable.RECTANGLE
        cornerRadius = dp(radiusDp).toFloat()
        setColor(fill)
        if (stroke != null) setStroke(dp(strokeDp), stroke)
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density + 0.5f).toInt()

    // ---------------------------------------------------------------- navigation chrome

    private fun refresh() {
        val t = cur
        homeScroll.visibility = if (t.showHome && t.error == null) View.VISIBLE else View.GONE
        webHost.visibility = if (!t.showHome && t.error == null && t.web != null) View.VISIBLE else View.GONE
        errorBox.visibility = if (t.error != null) View.VISIBLE else View.GONE
        if (t.error != null) errorDetail.text = t.error
        if (!t.showHome && t.web != null) {
            webHost.removeAllViews()
            webHost.addView(t.web, FrameLayout.LayoutParams(-1, -1))
        }
        updateChrome()
    }

    private fun updateChrome() {
        val t = cur
        val url = if (t.showHome) HOME else currentUrl()
        if (!address.hasFocus()) address.setText(displayFor(url))
        val secure = url.startsWith("https://")
        lockIcon.setImageResource(when {
            t.showHome -> R.drawable.ic_search
            secure -> R.drawable.ic_lock
            else -> R.drawable.ic_search
        })
        progress.visibility = if (t.progress in 1..99) View.VISIBLE else View.INVISIBLE
        progress.progress = t.progress
        backButton.isEnabled = t.showHome || t.web?.canGoBack() == true || t.error != null
        forwardButton.isEnabled = !t.showHome && t.web?.canGoForward() == true
        starButton.imageTintList = ColorStateList.valueOf(
            if (isBookmarked(url)) color(R.color.sky_accent) else color(R.color.sky_ink)
        )
        tabCount.text = tabs.size.toString()
    }

    private fun currentUrl(): String = cur.web?.url ?: cur.url

    private fun displayFor(url: String): String {
        if (url == HOME || url.startsWith("skylark://")) return ""
        return try {
            val u = Uri.parse(url)
            (u.host ?: url).removePrefix("www.")
        } catch (_: Exception) { url }
    }

    private fun focusAddress() {
        address.requestFocus()
        val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.showSoftInput(address, InputMethodManager.SHOW_IMPLICIT)
    }

    private fun releaseAddressFocus() {
        address.clearFocus()
        val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.hideSoftInputFromWindow(address.windowToken, 0)
    }

    private fun submitAddress() {
        val input = address.text.toString().trim()
        releaseAddressFocus()
        if (input.isBlank()) return
        val url = toUrl(input) ?: return
        navigate(url)
    }

    private fun toUrl(input: String): String? {
        val t = input.trim()
        if (t.isEmpty()) return null
        if (t.startsWith("skylark://")) return t
        if (t.contains(" ") || !t.contains(".")) return searchUrl(t)
        val withScheme = if (t.startsWith("http://") || t.startsWith("https://")) t else "https://$t"
        return withScheme
    }

    private fun searchUrl(query: String): String {
        val base = ENGINES.firstOrNull { it.first == prefs().getString("engine", "Google") }?.second ?: ENGINES[0].second
        return base + URLEncoder.encode(query, "UTF-8")
    }

    private fun navigate(url: String) {
        if (url == HOME || url.startsWith("skylark://home")) {
            cur.showHome = true
            cur.error = null
            refresh()
            return
        }
        cur.showHome = false
        cur.error = null
        cur.url = url
        val w = ensureWeb(cur)
        w.loadUrl(url)
        refresh()
    }

    private fun goBack() {
        when {
            cur.error != null -> { cur.error = null; refresh() }
            cur.showHome -> { /* already home */ }
            cur.web?.canGoBack() == true -> cur.web?.goBack()
            else -> { cur.showHome = true; refresh() }
        }
    }

    private fun goForward() {
        if (cur.web?.canGoForward() == true) cur.web?.goForward()
    }

    private fun onReloadOrStop() {
        val t = cur
        if (t.progress in 1..99) {
            t.web?.stopLoading()
            t.progress = 100
            updateChrome()
        } else if (t.error != null || !t.showHome) {
            t.error = null
            t.web?.reload() ?: navigate(t.url)
            refresh()
        }
    }

    // ---------------------------------------------------------------- tabs

    private fun ensureWeb(t: Tab): WebView {
        if (t.web == null) t.web = createWebView(t)
        return t.web!!
    }

    private fun showTab(i: Int) {
        if (i !in tabs.indices) return
        curIndex = i
        refresh()
    }

    private fun newTab() {
        tabs.add(Tab())
        curIndex = tabs.lastIndex
        refresh()
    }

    private fun openInNewTab(url: String) {
        val t = Tab()
        tabs.add(t)
        curIndex = tabs.lastIndex
        navigate(url)
    }

    private fun closeTab(i: Int) {
        if (tabs.size <= 1) {
            tabs[0].web?.destroy()
            tabs[0] = Tab()
            curIndex = 0
            refresh()
            return
        }
        tabs[i].web?.destroy()
        tabs.removeAt(i)
        if (curIndex >= tabs.size) curIndex = tabs.lastIndex
        else if (curIndex > i) curIndex--
        refresh()
    }

    private fun tabLabel(t: Tab): String {
        if (t.showHome) return "New tab"
        val title = t.title.ifBlank { displayFor(t.url) }
        return title.take(40).ifBlank { "Tab" }
    }

    private fun showTabs() {
        val labels = tabs.mapIndexed { i, t -> "${if (i == curIndex) "● " else ""}${tabLabel(t)}" }.toTypedArray()
        AlertDialog.Builder(this)
            .setTitle("Tabs (${tabs.size})")
            .setItems(labels) { _, which -> showTab(which) }
            .setPositiveButton("New tab") { _, _ -> newTab(); focusAddress() }
            .setNeutralButton("Close current") { _, _ -> closeTab(curIndex) }
            .setNegativeButton("Done", null)
            .show()
    }

    // ---------------------------------------------------------------- WebView

    private fun mobileUa(): String =
        WebSettings.getDefaultUserAgent(this).replace("; wv", "").replace("Version/4.0 ", "")

    private fun desktopUa(): String {
        val v = Regex("Chrome/([\\d.]+)").find(WebSettings.getDefaultUserAgent(this))?.groupValues?.get(1) ?: "120.0.0.0"
        return "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/$v Safari/537.36"
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun createWebView(tab: Tab): WebView {
        val w = WebView(this)
        val s = w.settings
        s.javaScriptEnabled = prefs().getBoolean("js_enabled", true)
        s.domStorageEnabled = true
        s.databaseEnabled = true
        s.mediaPlaybackRequiresUserGesture = false
        s.mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
        s.userAgentString = if (desktopMode) desktopUa() else mobileUa()
        s.setSupportZoom(true)
        s.builtInZoomControls = true
        s.displayZoomControls = false
        s.loadWithOverviewMode = true
        s.useWideViewPort = true
        s.allowFileAccess = true
        s.javaScriptCanOpenWindowsAutomatically = prefs().getBoolean("popups", false)
        s.setSupportMultipleWindows(false)
        CookieManager.getInstance().setAcceptCookie(prefs().getBoolean("cookies", true))
        CookieManager.getInstance().setAcceptThirdPartyCookies(w, prefs().getBoolean("third_party_cookies", true))

        w.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val u = request.url.toString()
                return when {
                    u.startsWith("http://") || u.startsWith("https://") -> false
                    u.startsWith("mailto:") || u.startsWith("tel:") || u.startsWith("sms:") -> {
                        try { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(u))) } catch (_: Exception) { }
                        true
                    }
                    else -> {
                        try { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(u))) } catch (_: Exception) { }
                        true
                    }
                }
            }

            override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                tab.url = url
                tab.progress = 10
                tab.error = null
                tab.showHome = false
                if (tab === cur) updateChrome()
            }

            override fun onPageFinished(view: WebView, url: String) {
                tab.url = url
                tab.progress = 100
                if (tab === cur) updateChrome()
                recordHistory(tab.title.ifBlank { displayFor(url) }, url)
            }

            override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) {
                tab.url = url
                if (tab === cur) updateChrome()
            }

            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (!request.isForMainFrame) return
                tab.error = friendlyError(error.description?.toString().orEmpty())
                tab.progress = 100
                if (tab === cur) refresh()
            }

            override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
                webHost.removeView(view)
                tab.web?.destroy()
                tab.web = null
                tab.error = "Page crashed. Tap Try again."
                if (tab === cur) refresh()
                return true
            }
        }

        w.webChromeClient = object : WebChromeClient() {
            override fun onProgressChanged(view: WebView, newProgress: Int) {
                tab.progress = newProgress
                if (tab === cur) updateChrome()
            }

            override fun onReceivedTitle(view: WebView, title: String?) {
                tab.title = title.orEmpty()
                if (tab === cur) updateChrome()
            }

            override fun onShowCustomView(view: View, callback: CustomViewCallback) {
                if (customView != null) { callback.onCustomViewHidden(); return }
                customView = view
                customCallback = callback
                root.addView(view, LinearLayout.LayoutParams(-1, -1))
                setImmersiveUi(true)
            }

            override fun onHideCustomView() { hideCustomView() }

            override fun getDefaultVideoPoster(): Bitmap? = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)

            override fun onShowFileChooser(webView: WebView, filePathCallback: ValueCallback<Array<Uri>>, params: FileChooserParams): Boolean {
                fileCallback?.onReceiveValue(null)
                fileCallback = filePathCallback
                val intent = params.createIntent()
                return try {
                    startActivityForResult(intent, REQ_FILE)
                    true
                } catch (_: Exception) {
                    fileCallback = null
                    false
                }
            }
        }

        w.setDownloadListener { url, userAgent, contentDisposition, mimeType, _ ->
            startDownload(url, userAgent, contentDisposition, mimeType)
        }

        w.setOnLongClickListener {
            val hr = w.hitTestResult
            val extra = hr.extra
            when (hr.type) {
                WebView.HitTestResult.SRC_ANCHOR_TYPE,
                WebView.HitTestResult.SRC_IMAGE_ANCHOR_TYPE -> {
                    if (extra != null) linkDialog(extra, false)
                    true
                }
                WebView.HitTestResult.IMAGE_TYPE -> {
                    if (extra != null) linkDialog(extra, true)
                    true
                }
                else -> false
            }
        }
        return w
    }

    private fun friendlyError(raw: String): String = when {
        raw.contains("ERR_INTERNET_DISCONNECTED", true) || raw.contains("NETWORK", true) -> "No internet connection"
        raw.contains("ERR_NAME_NOT_RESOLVED", true) -> "Site not found"
        raw.contains("ERR_CONNECTION", true) -> "Connection failed"
        raw.contains("ERR_TIMED_OUT", true) -> "Request timed out"
        raw.contains("ERR_SSL", true) || raw.contains("CERT", true) -> "Secure connection failed"
        else -> raw.ifBlank { "Unknown error" }
    }

    private fun linkDialog(url: String, isImage: Boolean) {
        val items = arrayOf("Open", "Open in new tab", "Copy link", "Share", if (isImage) "Download image" else "Download")
        AlertDialog.Builder(this)
            .setTitle(url.take(80))
            .setItems(items) { _, which ->
                when (which) {
                    0 -> navigate(url)
                    1 -> openInNewTab(url)
                    2 -> copyText(url)
                    3 -> shareText(url)
                    4 -> startDownload(url, null, null, null)
                }
            }
            .show()
    }

    private fun hideCustomView() {
        customView?.let { root.removeView(it) }
        customView = null
        customCallback?.onCustomViewHidden()
        customCallback = null
        setImmersiveUi(false)
    }

    private fun setImmersiveUi(on: Boolean) {
        if (Build.VERSION.SDK_INT >= 30) {
            val c = window.insetsController ?: return
            if (on) {
                c.hide(WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars())
                c.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            } else c.show(WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars())
        } else {
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = if (on)
                (View.SYSTEM_UI_FLAG_FULLSCREEN or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY)
            else View.SYSTEM_UI_FLAG_VISIBLE
        }
    }

    private fun startDownload(url: String, userAgent: String?, contentDisposition: String?, mime: String?) {
        try {
            val name = URLUtil.guessFileName(url, contentDisposition, mime)
            val req = DownloadManager.Request(Uri.parse(url)).apply {
                if (!mime.isNullOrBlank()) setMimeType(mime)
                if (!userAgent.isNullOrBlank()) addRequestHeader("User-Agent", userAgent)
                CookieManager.getInstance().getCookie(url)?.let { addRequestHeader("Cookie", it) }
                setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
                setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name)
            }
            (getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager).enqueue(req)
            Toast.makeText(this, "Downloading $name", Toast.LENGTH_SHORT).show()
        } catch (_: Exception) {
            Toast.makeText(this, "Can't download this file", Toast.LENGTH_SHORT).show()
        }
    }

    // ---------------------------------------------------------------- menu

    private fun showMenu(anchor: View) {
        val popup = PopupMenu(this, anchor)
        val m = popup.menu
        m.add(0, 1, 0, "New tab")
        m.add(0, 2, 1, "Desktop site").apply { isCheckable = true; isChecked = desktopMode }
        m.add(0, 12, 2, "Ask Skylark AI")
        m.add(0, 13, 3, "Summarize page")
        m.add(0, 14, 4, "Reader mode")
        m.add(0, 3, 5, "Find in page")
        m.add(0, 4, 6, "Share link")
        m.add(0, 5, 7, "Copy link")
        m.add(0, 6, 8, "History")
        m.add(0, 7, 9, "Bookmarks")
        m.add(0, 8, 10, "Downloads")
        m.add(0, 9, 11, "Settings")
        val user = accounts.currentUser()
        m.add(0, 10, 12, if (user != null) "Account · ${user.displayName}" else "Sign in / Sign up")
        m.add(0, 11, 13, "About Skylark")
        popup.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                1 -> { newTab(); focusAddress() }
                2 -> toggleDesktop()
                12 -> showAiChat()
                13 -> summarizeCurrentPage()
                14 -> toggleReaderMode()
                3 -> showFindInPage()
                4 -> shareText(currentUrl().takeIf { !cur.showHome })
                5 -> copyText(currentUrl().takeIf { !cur.showHome })
                6 -> showHistoryScreen()
                7 -> showBookmarksScreen()
                8 -> try { startActivity(Intent(DownloadManager.ACTION_VIEW_DOWNLOADS)) } catch (_: Exception) { }
                9 -> showSettingsScreen()
                10 -> if (user != null) showAccountScreen() else showLoginScreen()
                11 -> AlertDialog.Builder(this).setTitle("Skylark 0.6.0")
                    .setMessage("A clean, fast browser for Android with Skylark AI (Groq). Pages are drawn by the system WebView (Chromium).\n\nLocal accounts, full Settings, history, bookmarks and AI chat.")
                    .setPositiveButton("OK", null).show()
            }
            true
        }
        popup.show()
    }

    private fun toggleDesktop() {
        desktopMode = !desktopMode
        val ua = if (desktopMode) desktopUa() else mobileUa()
        for (t in tabs) t.web?.settings?.userAgentString = ua
        if (!cur.showHome) cur.web?.reload()
        Toast.makeText(this, if (desktopMode) "Desktop site on" else "Desktop site off", Toast.LENGTH_SHORT).show()
    }

    private fun shareText(text: String?) {
        if (text == null) { Toast.makeText(this, "Open a page first", Toast.LENGTH_SHORT).show(); return }
        val send = Intent(Intent.ACTION_SEND).apply { type = "text/plain"; putExtra(Intent.EXTRA_TEXT, text) }
        startActivity(Intent.createChooser(send, "Share link"))
    }

    private fun copyText(text: String?) {
        if (text == null) { Toast.makeText(this, "Open a page first", Toast.LENGTH_SHORT).show(); return }
        (getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Skylark link", text))
        Toast.makeText(this, "Link copied", Toast.LENGTH_SHORT).show()
    }

    private fun showFindInPage() {
        if (cur.showHome || cur.web == null) {
            Toast.makeText(this, "Open a page first", Toast.LENGTH_SHORT).show()
            return
        }
        val input = EditText(this).apply {
            hint = "Find in page"
            setSingleLine()
            setPadding(dp(16), dp(12), dp(16), dp(12))
        }
        AlertDialog.Builder(this)
            .setTitle("Find in page")
            .setView(input)
            .setPositiveButton("Find") { _, _ ->
                val q = input.text.toString()
                if (q.isNotBlank()) {
                    if (Build.VERSION.SDK_INT >= 33) cur.web?.findAllAsync(q)
                    else @Suppress("DEPRECATION") cur.web?.findAll(q)
                }
            }
            .setNeutralButton("Next") { _, _ -> cur.web?.findNext(true) }
            .setNegativeButton("Close") { _, _ -> cur.web?.clearMatches() }
            .show()
    }

    // ---------------------------------------------------------------- history

    private fun recordHistory(title: String, url: String) {
        if (url == HOME || url.startsWith("skylark://") || !url.startsWith("http")) return
        val list = loadHistory().toMutableList()
        list.removeAll { it.url == url }
        list.add(0, HistoryEntry(title.ifBlank { displayFor(url) }, url, System.currentTimeMillis()))
        while (list.size > MAX_HISTORY) list.removeAt(list.lastIndex)
        val arr = JSONArray()
        for (e in list) arr.put(JSONObject().put("title", e.title).put("url", e.url).put("ts", e.ts))
        prefs().edit().putString("history", arr.toString()).apply()
    }

    private fun loadHistory(): List<HistoryEntry> {
        val out = mutableListOf<HistoryEntry>()
        try {
            val arr = JSONArray(prefs().getString("history", "[]"))
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                out.add(HistoryEntry(o.getString("title"), o.getString("url"), o.getLong("ts")))
            }
        } catch (_: Exception) { }
        return out
    }

    private fun clearHistory() {
        prefs().edit().remove("history").apply()
    }

    // ---------------------------------------------------------------- shortcuts / bookmarks

    private fun defaultShortcuts() = listOf(
        Shortcut("Google", "https://www.google.com/", 0xFF4285F4.toInt(), false),
        Shortcut("YouTube", "https://m.youtube.com/", 0xFFE62117.toInt(), false),
        Shortcut("Wikipedia", "https://en.wikipedia.org/", 0xFF475569.toInt(), false),
        Shortcut("Reddit", "https://www.reddit.com/", 0xFFFF4500.toInt(), false),
        Shortcut("GitHub", "https://github.com/", 0xFF24292F.toInt(), false),
        Shortcut("Maps", "https://www.google.com/maps", 0xFF16A34A.toInt(), false),
        Shortcut("Hacker News", "https://news.ycombinator.com/", 0xFFFF6600.toInt(), false),
        Shortcut("BBC", "https://www.bbc.com/", 0xFFBB1919.toInt(), false)
    )

    private fun loadCustomShortcuts(): List<Shortcut> {
        val out = mutableListOf<Shortcut>()
        try {
            val arr = JSONArray(prefs().getString("shortcuts", "[]"))
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                val url = o.getString("url")
                out.add(Shortcut(o.getString("title"), url, TILE_COLORS[Math.floorMod(url.hashCode(), TILE_COLORS.size)], true))
            }
        } catch (_: Exception) { }
        return out
    }

    private fun saveCustomShortcuts(list: List<Shortcut>) {
        val arr = JSONArray()
        for (s in list) arr.put(JSONObject().put("title", s.title).put("url", s.url))
        prefs().edit().putString("shortcuts", arr.toString()).apply()
    }

    private fun isBookmarked(url: String) = url != HOME && loadCustomShortcuts().any { it.url == url }

    private fun toggleBookmark() {
        if (cur.showHome || cur.web == null) {
            Toast.makeText(this, "Open a page first, then tap the star", Toast.LENGTH_SHORT).show()
            return
        }
        val url = currentUrl()
        val list = loadCustomShortcuts().toMutableList()
        val existing = list.indexOfFirst { it.url == url }
        if (existing >= 0) {
            list.removeAt(existing)
            Toast.makeText(this, "Removed from start page", Toast.LENGTH_SHORT).show()
        } else {
            val title = cur.title.ifBlank { displayFor(url) }.take(24)
            list.add(Shortcut(title, url, 0, true))
            Toast.makeText(this, "Added to start page", Toast.LENGTH_SHORT).show()
        }
        saveCustomShortcuts(list)
        refreshShortcuts()
        updateChrome()
    }

    private fun removeCustomShortcut(url: String) {
        saveCustomShortcuts(loadCustomShortcuts().filter { it.url != url })
        refreshShortcuts()
        updateChrome()
    }

    // ---------------------------------------------------------------- overlay helpers

    private fun openOverlay(content: View) {
        overlay.removeAllViews()
        overlay.addView(content, FrameLayout.LayoutParams(-1, -1))
        overlay.visibility = View.VISIBLE
    }

    private fun closeOverlay() {
        overlay.removeAllViews()
        overlay.visibility = View.GONE
    }

    private fun overlayHeader(title: String, onBack: () -> Unit = { closeOverlay() }): LinearLayout {
        val bar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(8), dp(12), dp(16), dp(12))
            setBackgroundColor(color(R.color.sky_bg))
        }
        val back = iconButton(R.drawable.ic_back, "Back") { onBack() }
        bar.addView(back, LinearLayout.LayoutParams(dp(44), dp(44)))
        bar.addView(TextView(this).apply {
            text = title
            textSize = 18f
            setTextColor(color(R.color.sky_ink))
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            setPadding(dp(4), 0, 0, 0)
        }, LinearLayout.LayoutParams(0, -2, 1f))
        return bar
    }

    private fun sectionLabel(text: String): TextView = TextView(this).apply {
        this.text = text.uppercase(Locale.US)
        textSize = 12f
        setTextColor(color(R.color.sky_muted))
        typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        setPadding(dp(20), dp(20), dp(20), dp(8))
    }

    private fun settingsRow(title: String, subtitle: String? = null, trailing: String? = null, onClick: (() -> Unit)? = null): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(20), dp(14), dp(20), dp(14))
            background = rounded(color(R.color.sky_card), 0)
            if (onClick != null) setOnClickListener { onClick() }
        }
        val textCol = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        textCol.addView(TextView(this).apply {
            text = title
            textSize = 16f
            setTextColor(color(R.color.sky_ink))
        })
        if (!subtitle.isNullOrBlank()) {
            textCol.addView(TextView(this).apply {
                text = subtitle
                textSize = 13f
                setTextColor(color(R.color.sky_muted))
                setPadding(0, dp(2), 0, 0)
            })
        }
        row.addView(textCol, LinearLayout.LayoutParams(0, -2, 1f))
        if (trailing != null) {
            row.addView(TextView(this).apply {
                text = trailing
                textSize = 14f
                setTextColor(color(R.color.sky_accent))
            })
        }
        return row
    }

    private fun switchRow(title: String, subtitle: String?, key: String, default: Boolean, onChanged: ((Boolean) -> Unit)? = null): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(20), dp(10), dp(16), dp(10))
        }
        val textCol = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        textCol.addView(TextView(this).apply {
            text = title
            textSize = 16f
            setTextColor(color(R.color.sky_ink))
        })
        if (!subtitle.isNullOrBlank()) {
            textCol.addView(TextView(this).apply {
                text = subtitle
                textSize = 13f
                setTextColor(color(R.color.sky_muted))
                setPadding(0, dp(2), 0, 0)
            })
        }
        row.addView(textCol, LinearLayout.LayoutParams(0, -2, 1f))
        val sw = Switch(this).apply {
            isChecked = prefs().getBoolean(key, default)
            setOnCheckedChangeListener { _, checked ->
                prefs().edit().putBoolean(key, checked).apply()
                onChanged?.invoke(checked)
            }
        }
        row.addView(sw)
        return row
    }

    // ---------------------------------------------------------------- SETTINGS (full)

    private fun showSettingsScreen() {
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
        }
        col.addView(overlayHeader("Settings"))

        // —— Search
        col.addView(sectionLabel("Search"))
        col.addView(settingsRow(
            "Search engine",
            prefs().getString("engine", "Google"),
            "Change"
        ) { chooseEngine { showSettingsScreen() } })

        // —— Appearance
        col.addView(sectionLabel("Appearance"))
        val themeLabel = when (prefs().getString("theme_mode", "system")) {
            "light" -> "Light"
            "dark" -> "Dark"
            else -> "System"
        }
        col.addView(settingsRow("Theme", themeLabel, "Change") {
            val modes = arrayOf("System", "Light", "Dark")
            val keys = arrayOf("system", "light", "dark")
            val selected = keys.indexOf(prefs().getString("theme_mode", "system")).coerceAtLeast(0)
            AlertDialog.Builder(this)
                .setTitle("Theme")
                .setSingleChoiceItems(modes, selected) { d, which ->
                    prefs().edit().putString("theme_mode", keys[which]).apply()
                    d.dismiss()
                    Toast.makeText(this, "Theme will fully apply after restart", Toast.LENGTH_SHORT).show()
                    showSettingsScreen()
                }
                .setNegativeButton("Cancel", null)
                .show()
        })
        col.addView(switchRow("Desktop site by default", "Open new pages in desktop mode", "desktop_default", false) {
            desktopMode = it
        })

        // —— Homepage
        col.addView(sectionLabel("Homepage"))
        col.addView(settingsRow(
            "Homepage",
            prefs().getString("homepage", HOME)?.let { if (it == HOME) "Skylark start page" else it } ?: "Skylark start page",
            "Edit"
        ) {
            val input = EditText(this).apply {
                setText(prefs().getString("homepage", HOME))
                hint = "skylark://home or https://…"
                setSingleLine()
                setPadding(dp(20), dp(16), dp(20), dp(16))
            }
            AlertDialog.Builder(this)
                .setTitle("Homepage URL")
                .setView(input)
                .setPositiveButton("Save") { _, _ ->
                    val v = input.text.toString().trim().ifBlank { HOME }
                    prefs().edit().putString("homepage", v).apply()
                    showSettingsScreen()
                }
                .setNegativeButton("Cancel", null)
                .show()
        })

        // —— Privacy & content
        col.addView(sectionLabel("Privacy & content"))
        col.addView(switchRow("JavaScript", "Required for most modern sites", "js_enabled", true) { enabled ->
            for (t in tabs) t.web?.settings?.javaScriptEnabled = enabled
        })
        col.addView(switchRow("Cookies", "Allow first-party cookies", "cookies", true) { enabled ->
            CookieManager.getInstance().setAcceptCookie(enabled)
        })
        col.addView(switchRow("Third-party cookies", "May be used for ads and embeds", "third_party_cookies", true) { enabled ->
            for (t in tabs) t.web?.let { CookieManager.getInstance().setAcceptThirdPartyCookies(it, enabled) }
        })
        col.addView(switchRow("Allow pop-ups", "Open window.open() in same tab", "popups", false) { enabled ->
            for (t in tabs) t.web?.settings?.javaScriptCanOpenWindowsAutomatically = enabled
        })
        col.addView(switchRow("Do Not Track", "Send DNT header when supported", "dnt", false))
        col.addView(switchRow("Clear data on exit", "Cookies & cache when you leave the app", "clear_on_exit", false))

        // —— Data
        col.addView(sectionLabel("Data"))
        col.addView(settingsRow("Clear browsing data", "Cookies, cache, history, site data", "Clear") {
            confirmClearDetailed()
        })
        col.addView(settingsRow("Clear history only", "${loadHistory().size} entries", "Clear") {
            clearHistory()
            Toast.makeText(this, "History cleared", Toast.LENGTH_SHORT).show()
            showSettingsScreen()
        })

        // —— Downloads
        col.addView(sectionLabel("Downloads"))
        col.addView(settingsRow("Open downloads folder", null, "Open") {
            try { startActivity(Intent(DownloadManager.ACTION_VIEW_DOWNLOADS)) } catch (_: Exception) {
                Toast.makeText(this, "Downloads app not available", Toast.LENGTH_SHORT).show()
            }
        })

        // —— Skylark AI
        col.addView(sectionLabel("Skylark AI"))
        col.addView(switchRow("Enable Skylark AI", "Chat and page summaries via Groq", "ai_enabled", true))
        val modelLabel = AiHelper.MODELS.firstOrNull { it.first == prefs().getString("ai_model", "llama-3.1-8b-instant") }?.second
            ?: "Llama 3.1 8B (fast)"
        col.addView(settingsRow("Model", modelLabel, "Change") {
            val labels = AiHelper.MODELS.map { it.second }.toTypedArray()
            val keys = AiHelper.MODELS.map { it.first }
            val selected = keys.indexOf(prefs().getString("ai_model", "llama-3.1-8b-instant")).coerceAtLeast(0)
            AlertDialog.Builder(this)
                .setTitle("AI model")
                .setSingleChoiceItems(labels, selected) { d, which ->
                    prefs().edit().putString("ai_model", keys[which]).apply()
                    d.dismiss()
                    showSettingsScreen()
                }
                .setNegativeButton("Cancel", null)
                .show()
        })
        val keyHint = prefs().getString("ai_api_key", "")?.let {
            if (it.length > 12) it.take(7) + "…" + it.takeLast(4) else "(not set)"
        } ?: "(not set)"
        col.addView(settingsRow("API key (Groq)", keyHint, "Edit") {
            val input = EditText(this).apply {
                setText(prefs().getString("ai_api_key", AiHelper.DEFAULT_KEY))
                setSingleLine()
                inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
                setPadding(dp(20), dp(16), dp(20), dp(16))
            }
            AlertDialog.Builder(this)
                .setTitle("Groq API key")
                .setMessage("Stored only on this device. Anyone with the APK can extract it — use a key you can rotate.")
                .setView(input)
                .setPositiveButton("Save") { _, _ ->
                    prefs().edit().putString("ai_api_key", input.text.toString().trim()).apply()
                    showSettingsScreen()
                }
                .setNegativeButton("Cancel", null)
                .show()
        })

        // —— Account
        col.addView(sectionLabel("Account"))
        val user = accounts.currentUser()
        if (user != null) {
            col.addView(settingsRow("Signed in as ${user.displayName}", user.email, "Manage") { showAccountScreen() })
        } else {
            col.addView(settingsRow("Not signed in", "Create a local Skylark account", "Sign in") { showLoginScreen() })
        }

        // —— About
        col.addView(sectionLabel("About"))
        col.addView(settingsRow("Version", "0.6.0", null))
        col.addView(settingsRow("About Skylark", "Clean browser powered by system WebView", null) {
            AlertDialog.Builder(this).setTitle("Skylark 0.6.0")
                .setMessage("Pages are drawn by the system WebView (Chromium). Local accounts, Settings, history, bookmarks and Skylark AI (Groq) are stored only on this device.")
                .setPositiveButton("OK", null).show()
        })

        col.addView(View(this), LinearLayout.LayoutParams(-1, dp(40)))
        scroll.addView(col)
        openOverlay(scroll)
    }

    private fun chooseEngine(after: (() -> Unit)? = null) {
        val names = ENGINES.map { it.first }.toTypedArray()
        val selected = names.indexOf(prefs().getString("engine", "Google")).coerceAtLeast(0)
        AlertDialog.Builder(this)
            .setTitle("Search engine")
            .setSingleChoiceItems(names, selected) { d, which ->
                prefs().edit().putString("engine", names[which]).apply()
                d.dismiss()
                after?.invoke()
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun confirmClearDetailed() {
        val options = arrayOf("Cookies & site data", "Cached images and files", "Browsing history", "Everything")
        val checked = booleanArrayOf(true, true, false, false)
        AlertDialog.Builder(this)
            .setTitle("Clear browsing data")
            .setMultiChoiceItems(options, checked) { _, which, isChecked ->
                if (which == 3 && isChecked) {
                    checked[0] = true; checked[1] = true; checked[2] = true
                }
                checked[which] = isChecked
            }
            .setPositiveButton("Clear") { _, _ ->
                if (checked[0] || checked[3]) {
                    CookieManager.getInstance().removeAllCookies(null)
                    WebStorage.getInstance().deleteAllData()
                }
                if (checked[1] || checked[3]) {
                    for (t in tabs) t.web?.clearCache(true)
                }
                if (checked[2] || checked[3]) clearHistory()
                CookieManager.getInstance().flush()
                Toast.makeText(this, "Browsing data cleared", Toast.LENGTH_SHORT).show()
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    // ---------------------------------------------------------------- HISTORY screen

    private fun showHistoryScreen() {
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
        }
        col.addView(overlayHeader("History"))
        val hist = loadHistory()
        if (hist.isEmpty()) {
            col.addView(TextView(this).apply {
                text = "No history yet"
                textSize = 15f
                setTextColor(color(R.color.sky_muted))
                gravity = Gravity.CENTER
                setPadding(dp(24), dp(48), dp(24), dp(24))
            })
        } else {
            val df = SimpleDateFormat("MMM d · HH:mm", Locale.getDefault())
            for (e in hist) {
                col.addView(settingsRow(e.title, e.url + "\n" + df.format(Date(e.ts)), null) {
                    closeOverlay()
                    navigate(e.url)
                })
            }
            col.addView(settingsRow("Clear all history", null, "Clear") {
                clearHistory()
                Toast.makeText(this, "History cleared", Toast.LENGTH_SHORT).show()
                showHistoryScreen()
            })
        }
        col.addView(View(this), LinearLayout.LayoutParams(-1, dp(32)))
        scroll.addView(col)
        openOverlay(scroll)
    }

    // ---------------------------------------------------------------- BOOKMARKS screen

    private fun showBookmarksScreen() {
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
        }
        col.addView(overlayHeader("Bookmarks"))
        val customs = loadCustomShortcuts()
        if (customs.isEmpty()) {
            col.addView(TextView(this).apply {
                text = "No bookmarks yet.\nTap the star on any page to add it to the start page."
                textSize = 15f
                setTextColor(color(R.color.sky_muted))
                gravity = Gravity.CENTER
                setPadding(dp(24), dp(48), dp(24), dp(24))
            })
        } else {
            for (s in customs) {
                val row = settingsRow(s.title, s.url, "Open") {
                    closeOverlay()
                    navigate(s.url)
                }
                row.setOnLongClickListener {
                    AlertDialog.Builder(this)
                        .setTitle("Remove bookmark?")
                        .setMessage(s.title)
                        .setPositiveButton("Remove") { _, _ ->
                            removeCustomShortcut(s.url)
                            showBookmarksScreen()
                        }
                        .setNegativeButton("Cancel", null)
                        .show()
                    true
                }
                col.addView(row)
            }
        }
        col.addView(View(this), LinearLayout.LayoutParams(-1, dp(32)))
        scroll.addView(col)
        openOverlay(scroll)
    }

    // ---------------------------------------------------------------- AUTH: Login / Sign up / Account / Continue with Skylark

    private fun field(hint: String, password: Boolean = false): EditText = EditText(this).apply {
        this.hint = hint
        textSize = 16f
        setTextColor(color(R.color.sky_ink))
        setHintTextColor(color(R.color.sky_muted))
        setSingleLine()
        background = rounded(color(R.color.sky_pill), 14)
        setPadding(dp(16), dp(14), dp(16), dp(14))
        if (password) {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
        } else {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
        }
    }

    private fun primaryButton(label: String, onClick: () -> Unit): TextView = TextView(this).apply {
        text = label
        textSize = 16f
        setTextColor(Color.WHITE)
        gravity = Gravity.CENTER
        typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        background = rounded(color(R.color.sky_accent), 14)
        setPadding(dp(20), dp(14), dp(20), dp(14))
        setOnClickListener { onClick() }
    }

    private fun secondaryButton(label: String, onClick: () -> Unit): TextView = TextView(this).apply {
        text = label
        textSize = 15f
        setTextColor(color(R.color.sky_accent))
        gravity = Gravity.CENTER
        setPadding(dp(16), dp(12), dp(16), dp(12))
        setOnClickListener { onClick() }
    }

    private fun showLoginScreen() {
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
            setPadding(dp(24), dp(16), dp(24), dp(32))
        }
        col.addView(overlayHeader("Sign in"))
        col.addView(TextView(this).apply {
            text = "Welcome back"
            textSize = 24f
            setTextColor(color(R.color.sky_ink))
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            setPadding(0, dp(16), 0, dp(8))
        })
        col.addView(TextView(this).apply {
            text = "Use the username or email you registered with. Accounts stay on this device."
            textSize = 14f
            setTextColor(color(R.color.sky_muted))
            setPadding(0, 0, 0, dp(24))
        })
        val idField = field("Username or email")
        val passField = field("Password", password = true)
        col.addView(idField, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(12) })
        col.addView(passField, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(20) })
        col.addView(primaryButton("Sign in") {
            val r = accounts.login(idField.text.toString(), passField.text.toString())
            r.onSuccess {
                Toast.makeText(this, "Signed in as ${it.displayName}", Toast.LENGTH_SHORT).show()
                closeOverlay()
                refreshShortcuts()
                // rebuild home greeting
                homeScroll.removeAllViews()
                homeScroll.addView(buildHome())
            }.onFailure {
                Toast.makeText(this, it.message ?: "Sign in failed", Toast.LENGTH_SHORT).show()
            }
        }, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(12) })
        col.addView(secondaryButton("Create a Skylark account") { showSignupScreen() })
        scroll.addView(col)
        openOverlay(scroll)
    }

    private fun showSignupScreen() {
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
            setPadding(dp(24), dp(16), dp(24), dp(32))
        }
        col.addView(overlayHeader("Create account") { showLoginScreen() })
        col.addView(TextView(this).apply {
            text = "Join Skylark"
            textSize = 24f
            setTextColor(color(R.color.sky_ink))
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            setPadding(0, dp(16), 0, dp(8))
        })
        col.addView(TextView(this).apply {
            text = "Your account is stored only on this phone. No servers, no tracking."
            textSize = 14f
            setTextColor(color(R.color.sky_muted))
            setPadding(0, 0, 0, dp(24))
        })
        val nameField = field("Display name")
        val userField = field("Username")
        val emailField = field("Email")
        val passField = field("Password (min 6 characters)", password = true)
        listOf(nameField, userField, emailField, passField).forEach {
            col.addView(it, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(12) })
        }
        col.addView(primaryButton("Create account") {
            val r = accounts.signUp(
                username = userField.text.toString(),
                email = emailField.text.toString(),
                password = passField.text.toString(),
                displayName = nameField.text.toString()
            )
            r.onSuccess { user ->
                showContinueWithSkylark(user)
            }.onFailure {
                Toast.makeText(this, it.message ?: "Could not create account", Toast.LENGTH_SHORT).show()
            }
        }, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(8); bottomMargin = dp(12) })
        col.addView(secondaryButton("Already have an account? Sign in") { showLoginScreen() })
        scroll.addView(col)
        openOverlay(scroll)
    }

    /** Post-signup branded welcome — "Continue with Skylark" */
    private fun showContinueWithSkylark(user: AccountManager.User) {
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setBackgroundColor(color(R.color.sky_bg))
            setPadding(dp(32), dp(48), dp(32), dp(32))
        }
        col.addView(ImageView(this).apply {
            setImageResource(R.drawable.ic_logo_mark)
            scaleType = ImageView.ScaleType.FIT_CENTER
        }, LinearLayout.LayoutParams(dp(88), dp(88)).apply { bottomMargin = dp(16) })
        col.addView(TextView(this).apply {
            text = "You're in, ${user.displayName}"
            textSize = 24f
            setTextColor(color(R.color.sky_ink))
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            gravity = Gravity.CENTER
        })
        col.addView(TextView(this).apply {
            text = "Your Skylark account is ready on this device.\nBookmarks and preferences stay private."
            textSize = 15f
            setTextColor(color(R.color.sky_muted))
            gravity = Gravity.CENTER
            setPadding(0, dp(12), 0, dp(32))
        })
        col.addView(primaryButton("Continue with Skylark") {
            closeOverlay()
            homeScroll.removeAllViews()
            homeScroll.addView(buildHome())
            refreshShortcuts()
            Toast.makeText(this, "Welcome to Skylark", Toast.LENGTH_SHORT).show()
        }, LinearLayout.LayoutParams(-1, -2))
        openOverlay(col)
    }

    private fun showAccountScreen() {
        val user = accounts.currentUser() ?: run {
            showLoginScreen()
            return
        }
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
        }
        col.addView(overlayHeader("Account"))
        // Profile card
        val card = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = rounded(color(R.color.sky_card), 16)
            setPadding(dp(20), dp(20), dp(20), dp(20))
        }
        val avatar = TextView(this).apply {
            text = user.displayName.firstOrNull()?.uppercaseChar()?.toString() ?: "?"
            textSize = 28f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            background = rounded(color(R.color.sky_accent), 32)
        }
        card.addView(avatar, LinearLayout.LayoutParams(dp(64), dp(64)).apply {
            gravity = Gravity.CENTER_HORIZONTAL
            bottomMargin = dp(12)
        })
        card.addView(TextView(this).apply {
            text = user.displayName
            textSize = 20f
            setTextColor(color(R.color.sky_ink))
            gravity = Gravity.CENTER
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        })
        card.addView(TextView(this).apply {
            text = "@${user.username}"
            textSize = 14f
            setTextColor(color(R.color.sky_muted))
            gravity = Gravity.CENTER
            setPadding(0, dp(4), 0, 0)
        })
        card.addView(TextView(this).apply {
            text = user.email
            textSize = 13f
            setTextColor(color(R.color.sky_muted))
            gravity = Gravity.CENTER
            setPadding(0, dp(2), 0, 0)
        })
        col.addView(card, LinearLayout.LayoutParams(-1, -2).apply {
            leftMargin = dp(16); rightMargin = dp(16); topMargin = dp(8); bottomMargin = dp(8)
        })

        col.addView(sectionLabel("Profile"))
        col.addView(settingsRow("Edit display name", user.displayName, "Edit") {
            val input = EditText(this).apply {
                setText(user.displayName)
                setSingleLine()
                setPadding(dp(20), dp(16), dp(20), dp(16))
            }
            AlertDialog.Builder(this)
                .setTitle("Display name")
                .setView(input)
                .setPositiveButton("Save") { _, _ ->
                    accounts.updateDisplayName(input.text.toString())
                    showAccountScreen()
                    homeScroll.removeAllViews()
                    homeScroll.addView(buildHome())
                }
                .setNegativeButton("Cancel", null)
                .show()
        })

        col.addView(sectionLabel("Session"))
        col.addView(settingsRow("Sign out", "Stay signed out until you sign in again", "Sign out") {
            AlertDialog.Builder(this)
                .setTitle("Sign out?")
                .setMessage("You can sign back in any time with the same username and password.")
                .setPositiveButton("Sign out") { _, _ ->
                    accounts.logout()
                    closeOverlay()
                    homeScroll.removeAllViews()
                    homeScroll.addView(buildHome())
                    Toast.makeText(this, "Signed out", Toast.LENGTH_SHORT).show()
                }
                .setNegativeButton("Cancel", null)
                .show()
        })
        col.addView(settingsRow("Delete account", "Remove this account from the device", "Delete") {
            AlertDialog.Builder(this)
                .setTitle("Delete account?")
                .setMessage("This permanently removes your local Skylark account. Bookmarks and history are not deleted.")
                .setPositiveButton("Delete") { _, _ ->
                    accounts.deleteAccount()
                    closeOverlay()
                    homeScroll.removeAllViews()
                    homeScroll.addView(buildHome())
                    Toast.makeText(this, "Account deleted", Toast.LENGTH_SHORT).show()
                }
                .setNegativeButton("Cancel", null)
                .show()
        })

        col.addView(View(this), LinearLayout.LayoutParams(-1, dp(40)))
        scroll.addView(col)
        openOverlay(scroll)
    }


    // ---------------------------------------------------------------- Skylark AI + extras

    private val aiHistory = mutableListOf<AiHelper.Message>()

    private fun showAiChat(seedPrompt: String? = null) {
        if (!ai.enabled()) {
            AlertDialog.Builder(this)
                .setTitle("Skylark AI")
                .setMessage("Set a Groq API key in Settings → Skylark AI, or turn the feature on.")
                .setPositiveButton("Open Settings") { _, _ -> showSettingsScreen() }
                .setNegativeButton("Cancel", null)
                .show()
            return
        }
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.sky_bg))
        }
        col.addView(overlayHeader("Skylark AI") {
            closeOverlay()
        })

        val messagesBox = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(12), dp(8), dp(12), dp(8))
        }
        val scroll = ScrollView(this).apply {
            isFillViewport = true
            addView(messagesBox)
        }
        col.addView(scroll, LinearLayout.LayoutParams(-1, 0, 1f))

        fun bubble(text: String, fromUser: Boolean): View {
            val row = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = if (fromUser) Gravity.END else Gravity.START
                setPadding(dp(4), dp(4), dp(4), dp(4))
            }
            val tv = TextView(this).apply {
                this.text = text
                textSize = 15f
                setTextColor(if (fromUser) Color.WHITE else color(R.color.sky_ink))
                background = rounded(
                    if (fromUser) color(R.color.sky_accent) else color(R.color.sky_card),
                    16
                )
                setPadding(dp(14), dp(10), dp(14), dp(10))
                maxWidth = (resources.displayMetrics.widthPixels * 0.82).toInt()
            }
            row.addView(tv)
            return row
        }

        fun addBubble(text: String, fromUser: Boolean) {
            messagesBox.addView(bubble(text, fromUser))
            scroll.post { scroll.fullScroll(View.FOCUS_DOWN) }
        }

        // Replay history
        for (m in aiHistory) {
            addBubble(m.content, m.role == "user")
        }
        if (aiHistory.isEmpty()) {
            addBubble("Hi — I'm Skylark AI. Ask anything, or open a page and use Summarize page from the menu.", false)
        }

        val inputRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(10), dp(8), dp(10), dp(10))
            setBackgroundColor(color(R.color.sky_bg))
        }
        val input = EditText(this).apply {
            hint = "Message Skylark AI…"
            textSize = 15f
            setTextColor(color(R.color.sky_ink))
            setHintTextColor(color(R.color.sky_muted))
            background = rounded(color(R.color.sky_pill), 22)
            setPadding(dp(16), dp(12), dp(16), dp(12))
            maxLines = 4
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES or InputType.TYPE_TEXT_FLAG_MULTI_LINE
        }
        inputRow.addView(input, LinearLayout.LayoutParams(0, -2, 1f))
        val send = TextView(this).apply {
            text = "Send"
            textSize = 14f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            background = rounded(color(R.color.sky_accent), 18)
            setPadding(dp(16), dp(12), dp(16), dp(12))
        }
        inputRow.addView(send, LinearLayout.LayoutParams(-2, -2).apply { leftMargin = dp(8) })
        col.addView(inputRow)

        var busy = false
        fun sendMessage(raw: String) {
            val q = raw.trim()
            if (q.isEmpty() || busy) return
            busy = true
            input.setText("")
            addBubble(q, true)
            aiHistory.add(AiHelper.Message("user", q))
            val thinking = bubble("…", false)
            messagesBox.addView(thinking)
            scroll.post { scroll.fullScroll(View.FOCUS_DOWN) }

            val pageCtx = buildPageContext()
            ai.chat(aiHistory.dropLast(1), q, pageCtx) { content, done, error ->
                messagesBox.removeView(thinking)
                if (error != null) {
                    addBubble("Error: $error", false)
                } else if (content != null) {
                    addBubble(content, false)
                    aiHistory.add(AiHelper.Message("assistant", content))
                    // Keep history bounded
                    while (aiHistory.size > 20) aiHistory.removeAt(0)
                }
                busy = false
            }
        }

        send.setOnClickListener { sendMessage(input.text.toString()) }
        input.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_SEND || actionId == EditorInfo.IME_ACTION_DONE) {
                sendMessage(input.text.toString()); true
            } else false
        }

        openOverlay(col)

        if (!seedPrompt.isNullOrBlank()) {
            input.post { sendMessage(seedPrompt) }
        }
    }

    private fun buildPageContext(): String? {
        if (cur.showHome || cur.web == null) return null
        val title = cur.title.ifBlank { displayFor(currentUrl()) }
        val url = currentUrl()
        return "Title: $title\nURL: $url"
    }

    private fun summarizeCurrentPage() {
        if (cur.showHome || cur.web == null) {
            Toast.makeText(this, "Open a page first", Toast.LENGTH_SHORT).show()
            return
        }
        if (!ai.enabled()) {
            Toast.makeText(this, "Enable Skylark AI in Settings first", Toast.LENGTH_SHORT).show()
            return
        }
        val thinkingToast = Toast.makeText(this, "Summarizing…", Toast.LENGTH_SHORT)
        thinkingToast.show()
        cur.web?.evaluateJavascript(
            "(function(){try{return (document.body && document.body.innerText || '').slice(0,3000);}catch(e){return '';}})()"
        ) { raw ->
            val body = try {
                if (raw == null || raw == "null") ""
                else org.json.JSONArray("[$raw]").getString(0)
            } catch (_: Exception) {
                raw?.trim()?.trim('"')?.replace("\\n", "\n") ?: ""
            }
            val ctx = buildString {
                appendLine("Title: ${cur.title.ifBlank { displayFor(currentUrl()) }}")
                appendLine("URL: ${currentUrl()}")
                if (body.isNotBlank()) {
                    appendLine()
                    appendLine("Page text excerpt:")
                    append(body.take(2800))
                }
            }
            ai.chat(emptyList(), "Summarize this page in clear bullet points. Focus on the main ideas and any key facts.", ctx) { content, _, error ->
                thinkingToast.cancel()
                if (error != null) {
                    AlertDialog.Builder(this).setTitle("Summary failed").setMessage(error)
                        .setPositiveButton("OK", null).show()
                } else {
                    AlertDialog.Builder(this)
                        .setTitle("Page summary")
                        .setMessage(content ?: "(empty)")
                        .setPositiveButton("OK", null)
                        .setNeutralButton("Open in AI chat") { _, _ ->
                            if (!content.isNullOrBlank()) {
                                aiHistory.add(AiHelper.Message("user", "Summarize this page"))
                                aiHistory.add(AiHelper.Message("assistant", content))
                            }
                            showAiChat()
                        }
                        .show()
                }
            }
        }
    }

    private fun toggleReaderMode() {
        if (cur.showHome || cur.web == null) {
            Toast.makeText(this, "Open a page first", Toast.LENGTH_SHORT).show()
            return
        }
        val js = """
            (function(){
              if (document.getElementById('skylark-reader')) {
                document.getElementById('skylark-reader').remove();
                return 'off';
              }
              var s = document.createElement('style');
              s.id = 'skylark-reader';
              s.textContent = 'body{max-width:40rem!important;margin:1.5rem auto!important;padding:0 1.2rem!important;font-size:1.15rem!important;line-height:1.65!important;background:#fafafa!important;color:#111!important;} img,video{max-width:100%!important;height:auto!important;} nav,header,footer,aside,.sidebar,.ad,.ads,.advertisement,[role=banner],[role=navigation]{display:none!important;}';
              document.head.appendChild(s);
              return 'on';
            })();
        """.trimIndent()
        cur.web?.evaluateJavascript(js) { result ->
            val on = result?.contains("on") == true
            Toast.makeText(this, if (on) "Reader mode on" else "Reader mode off", Toast.LENGTH_SHORT).show()
        }
    }
}
