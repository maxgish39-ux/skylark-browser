package com.skylark.browser

import android.content.Context
import android.os.Handler
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors

/**
 * Minimal Groq (OpenAI-compatible) client for Skylark AI.
 * Runs network work on a background executor and posts results to the main thread.
 */
class AiHelper(private val context: Context) {
    private val executor = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private val prefs = context.getSharedPreferences("skylark", Context.MODE_PRIVATE)

    data class Message(val role: String, val content: String)

    fun apiKey(): String = prefs.getString("ai_api_key", DEFAULT_KEY)?.trim().orEmpty()
    fun model(): String = prefs.getString("ai_model", "llama-3.1-8b-instant") ?: "llama-3.1-8b-instant"
    fun enabled(): Boolean = prefs.getBoolean("ai_enabled", true) && apiKey().isNotBlank()

    fun setApiKey(key: String) {
        prefs.edit().putString("ai_api_key", key.trim()).apply()
    }

    fun chat(
        history: List<Message>,
        userMessage: String,
        pageContext: String? = null,
        onTokenOrDone: (partial: String?, done: Boolean, error: String?) -> Unit
    ) {
        if (!enabled()) {
            onTokenOrDone(null, true, "AI is disabled or no API key is set. Open Settings → Skylark AI.")
            return
        }
        executor.execute {
            try {
                val messages = JSONArray()
                val system = buildString {
                    append("You are Skylark AI, a helpful, concise assistant built into the Skylark Android browser. ")
                    append("Answer clearly. Use short paragraphs or bullet points when useful. ")
                    append("If the user asks about the current page, use the page context provided. ")
                    append("Do not invent URLs or claim you can browse unless context is given.")
                    if (!pageContext.isNullOrBlank()) {
                        append("\n\nCurrent page context:\n")
                        append(pageContext.take(4000))
                    }
                }
                messages.put(JSONObject().put("role", "system").put("content", system))
                for (m in history) {
                    messages.put(JSONObject().put("role", m.role).put("content", m.content))
                }
                messages.put(JSONObject().put("role", "user").put("content", userMessage))

                val body = JSONObject()
                    .put("model", model())
                    .put("messages", messages)
                    .put("temperature", 0.7)
                    .put("max_tokens", 1024)
                    .put("stream", false)

                val url = URL("https://api.groq.com/openai/v1/chat/completions")
                val conn = (url.openConnection() as HttpURLConnection).apply {
                    requestMethod = "POST"
                    setRequestProperty("Authorization", "Bearer ${apiKey()}")
                    setRequestProperty("Content-Type", "application/json")
                    setRequestProperty("Accept", "application/json")
                    doOutput = true
                    connectTimeout = 20000
                    readTimeout = 60000
                }
                OutputStreamWriter(conn.outputStream, Charsets.UTF_8).use { it.write(body.toString()) }

                val code = conn.responseCode
                val stream = if (code in 200..299) conn.inputStream else conn.errorStream
                val text = BufferedReader(InputStreamReader(stream, Charsets.UTF_8)).use { it.readText() }
                conn.disconnect()

                if (code !in 200..299) {
                    val errMsg = try {
                        JSONObject(text).optJSONObject("error")?.optString("message")
                            ?: text.take(200)
                    } catch (_: Exception) { text.take(200) }
                    main.post { onTokenOrDone(null, true, "API error ($code): $errMsg") }
                    return@execute
                }

                val json = JSONObject(text)
                val content = json.getJSONArray("choices")
                    .getJSONObject(0)
                    .getJSONObject("message")
                    .getString("content")
                    .trim()
                main.post { onTokenOrDone(content, true, null) }
            } catch (e: Exception) {
                main.post { onTokenOrDone(null, true, e.message ?: "Network error") }
            }
        }
    }

    companion object {
        // Pre-filled from the key the project owner supplied. Users should replace it with their own
        // in Settings — client-side keys can be extracted from the APK.
        const val DEFAULT_KEY = ""
        val MODELS = listOf(
            "llama-3.1-8b-instant" to "Llama 3.1 8B (fast)",
            "llama-3.3-70b-versatile" to "Llama 3.3 70B (strong)",
            "mixtral-8x7b-32768" to "Mixtral 8x7B",
            "gemma2-9b-it" to "Gemma 2 9B"
        )
    }
}
