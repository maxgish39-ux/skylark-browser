package com.skylark.browser

import android.content.Context
import android.content.SharedPreferences
import org.json.JSONArray
import org.json.JSONObject
import java.security.MessageDigest

/**
 * Local-only account storage for Skylark.
 * Users, current session and a simple password hash live in SharedPreferences.
 * Not a replacement for a real identity provider — suitable for a personal browser profile.
 */
class AccountManager(private val context: Context) {
    private val prefs: SharedPreferences =
        context.getSharedPreferences("skylark_accounts", Context.MODE_PRIVATE)

    data class User(
        val id: String,
        val username: String,
        val email: String,
        val displayName: String,
        val createdAt: Long
    )

    fun isLoggedIn(): Boolean = prefs.getString("current_user_id", null) != null

    fun currentUser(): User? {
        val id = prefs.getString("current_user_id", null) ?: return null
        return loadUsers().find { it.id == id }
    }

    fun login(identifier: String, password: String): Result<User> {
        val users = loadUsersRaw()
        val key = identifier.trim().lowercase()
        for (i in 0 until users.length()) {
            val o = users.getJSONObject(i)
            val username = o.getString("username")
            val email = o.getString("email")
            if (username.equals(key, ignoreCase = true) || email.equals(key, ignoreCase = true)) {
                val hash = o.getString("password_hash")
                if (hash == hashPassword(password, o.getString("salt"))) {
                    prefs.edit().putString("current_user_id", o.getString("id")).apply()
                    return Result.success(
                        User(
                            o.getString("id"),
                            username,
                            email,
                            o.optString("display_name", username),
                            o.getLong("created_at")
                        )
                    )
                }
                return Result.failure(Exception("Wrong password"))
            }
        }
        return Result.failure(Exception("No account found for that username or email"))
    }

    fun signUp(
        username: String,
        email: String,
        password: String,
        displayName: String = username
    ): Result<User> {
        val u = username.trim()
        val e = email.trim().lowercase()
        if (u.length < 3) return Result.failure(Exception("Username must be at least 3 characters"))
        if (!e.contains("@") || e.length < 5) return Result.failure(Exception("Enter a valid email"))
        if (password.length < 6) return Result.failure(Exception("Password must be at least 6 characters"))

        val users = loadUsersRaw()
        for (i in 0 until users.length()) {
            val o = users.getJSONObject(i)
            if (o.getString("username").equals(u, ignoreCase = true))
                return Result.failure(Exception("Username already taken"))
            if (o.getString("email").equals(e, ignoreCase = true))
                return Result.failure(Exception("Email already registered"))
        }

        val id = "u_${System.currentTimeMillis()}"
        val salt = randomSalt()
        val hash = hashPassword(password, salt)
        val now = System.currentTimeMillis()
        val obj = JSONObject()
            .put("id", id)
            .put("username", u)
            .put("email", e)
            .put("display_name", displayName.trim().ifBlank { u })
            .put("password_hash", hash)
            .put("salt", salt)
            .put("created_at", now)
        users.put(obj)
        prefs.edit()
            .putString("users", users.toString())
            .putString("current_user_id", id)
            .apply()
        return Result.success(User(id, u, e, displayName.trim().ifBlank { u }, now))
    }

    fun logout() {
        prefs.edit().remove("current_user_id").apply()
    }

    fun updateDisplayName(name: String) {
        val id = prefs.getString("current_user_id", null) ?: return
        val users = loadUsersRaw()
        for (i in 0 until users.length()) {
            val o = users.getJSONObject(i)
            if (o.getString("id") == id) {
                o.put("display_name", name.trim().ifBlank { o.getString("username") })
                break
            }
        }
        prefs.edit().putString("users", users.toString()).apply()
    }

    fun deleteAccount() {
        val id = prefs.getString("current_user_id", null) ?: return
        val users = loadUsersRaw()
        val kept = JSONArray()
        for (i in 0 until users.length()) {
            val o = users.getJSONObject(i)
            if (o.getString("id") != id) kept.put(o)
        }
        prefs.edit()
            .putString("users", kept.toString())
            .remove("current_user_id")
            .apply()
    }

    private fun loadUsers(): List<User> {
        val list = mutableListOf<User>()
        val arr = loadUsersRaw()
        for (i in 0 until arr.length()) {
            val o = arr.getJSONObject(i)
            list.add(
                User(
                    o.getString("id"),
                    o.getString("username"),
                    o.getString("email"),
                    o.optString("display_name", o.getString("username")),
                    o.getLong("created_at")
                )
            )
        }
        return list
    }

    private fun loadUsersRaw(): JSONArray {
        return try {
            JSONArray(prefs.getString("users", "[]"))
        } catch (_: Exception) {
            JSONArray()
        }
    }

    private fun hashPassword(password: String, salt: String): String {
        val md = MessageDigest.getInstance("SHA-256")
        val bytes = md.digest("$salt:$password".toByteArray(Charsets.UTF_8))
        return bytes.joinToString("") { "%02x".format(it) }
    }

    private fun randomSalt(): String {
        val chars = "abcdefghijklmnopqrstuvwxyz0123456789"
        return (1..16).map { chars.random() }.joinToString("")
    }
}
