#include "common.h"
#include <mbedtls/pk.h>
#include <mbedtls/entropy.h>
#include <mbedtls/ctr_drbg.h>
#include <mbedtls/md.h>
#include <mbedtls/gcm.h>
#include <mbedtls/error.h>

// Helper to initialize RNG
static int get_rng(mbedtls_entropy_context* entropy, mbedtls_ctr_drbg_context* ctr_drbg) {
    mbedtls_entropy_init(entropy);
    mbedtls_ctr_drbg_init(ctr_drbg);
    const char* pers = "llm_crypto_rng";
    int ret = mbedtls_ctr_drbg_seed(ctr_drbg, mbedtls_entropy_func, entropy,
                                    (const unsigned char *)pers, strlen(pers));
    return ret;
}

static void free_rng(mbedtls_entropy_context* entropy, mbedtls_ctr_drbg_context* ctr_drbg) {
    mbedtls_ctr_drbg_free(ctr_drbg);
    mbedtls_entropy_free(entropy);
}

// Unified sign operation
long crypto_sign(long key_ptr, long data_ptr) {
    char* key_str = (char*)key_ptr;
    char* data_str = (char*)data_ptr;
    if (!key_str || !data_str) return 0;

    mbedtls_pk_context pk;
    mbedtls_pk_init(&pk);
    
    mbedtls_entropy_context entropy;
    mbedtls_ctr_drbg_context ctr_drbg;
    if (get_rng(&entropy, &ctr_drbg) != 0) {
        mbedtls_pk_free(&pk);
        return 0;
    }

    // Parse private key (supports PEM and DER)
    int ret = mbedtls_pk_parse_key(&pk, (const unsigned char*)key_str, strlen(key_str) + 1, NULL, 0,
                                   mbedtls_ctr_drbg_random, &ctr_drbg);
    if (ret != 0) {
        free_rng(&entropy, &ctr_drbg);
        mbedtls_pk_free(&pk);
        return 0;
    }

    // Hash the data (SHA-256 for modern ECC)
    unsigned char hash[32];
    mbedtls_md_context_t md_ctx;
    mbedtls_md_init(&md_ctx);
    mbedtls_md_setup(&md_ctx, mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), 0);
    mbedtls_md_starts(&md_ctx);
    mbedtls_md_update(&md_ctx, (const unsigned char*)data_str, strlen(data_str));
    mbedtls_md_finish(&md_ctx, hash);
    mbedtls_md_free(&md_ctx);

    unsigned char sig[MBEDTLS_PK_SIGNATURE_MAX_SIZE];
    size_t sig_len = 0;

    ret = mbedtls_pk_sign(&pk, MBEDTLS_MD_SHA256, hash, sizeof(hash), sig, sizeof(sig), &sig_len,
                          mbedtls_ctr_drbg_random, &ctr_drbg);

    free_rng(&entropy, &ctr_drbg);
    mbedtls_pk_free(&pk);

    if (ret != 0) return 0;

    // Convert to hex string for easy passing in llmlang
    char* out_str = llm_rt_alloc(sig_len * 2 + 1, RT_TYPE_STRING);
    for (size_t i = 0; i < sig_len; i++) {
        sprintf(out_str + (i * 2), "%02x", sig[i]);
    }
    out_str[sig_len * 2] = '\0';
    return (long)out_str;
}

// Unified verify operation
long crypto_verify(long key_ptr, long sig_ptr, long data_ptr) {
    char* key_str = (char*)key_ptr;
    char* sig_hex = (char*)sig_ptr;
    char* data_str = (char*)data_ptr;
    if (!key_str || !sig_hex || !data_str) return 0;

    size_t hex_len = strlen(sig_hex);
    if (hex_len % 2 != 0 || hex_len / 2 > MBEDTLS_PK_SIGNATURE_MAX_SIZE) return 0;
    
    size_t sig_len = hex_len / 2;
    unsigned char sig[MBEDTLS_PK_SIGNATURE_MAX_SIZE];
    for (size_t i = 0; i < sig_len; i++) {
        unsigned int val;
        sscanf(sig_hex + 2 * i, "%02x", &val);
        sig[i] = (unsigned char)val;
    }

    mbedtls_pk_context pk;
    mbedtls_pk_init(&pk);
    
    // Parse public key
    int ret = mbedtls_pk_parse_public_key(&pk, (const unsigned char*)key_str, strlen(key_str) + 1);
    if (ret != 0) {
        mbedtls_pk_free(&pk);
        return 0;
    }

    // Hash the data
    unsigned char hash[32];
    mbedtls_md_context_t md_ctx;
    mbedtls_md_init(&md_ctx);
    mbedtls_md_setup(&md_ctx, mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), 0);
    mbedtls_md_starts(&md_ctx);
    mbedtls_md_update(&md_ctx, (const unsigned char*)data_str, strlen(data_str));
    mbedtls_md_finish(&md_ctx, hash);
    mbedtls_md_free(&md_ctx);

    ret = mbedtls_pk_verify(&pk, MBEDTLS_MD_SHA256, hash, sizeof(hash), sig, sig_len);
    mbedtls_pk_free(&pk);

    return ret == 0 ? 1 : 0;
}

// AES-GCM Encrypt
// key_ptr: 32-byte hex string (256-bit key)
long crypto_encrypt(long key_ptr, long data_ptr) {
    char* key_hex = (char*)key_ptr;
    char* data_str = (char*)data_ptr;
    if (!key_hex || !data_str) return 0;
    
    if (strlen(key_hex) != 64) return 0; // Require 256-bit key hex

    unsigned char key[32];
    for (int i = 0; i < 32; i++) {
        unsigned int val;
        sscanf(key_hex + 2 * i, "%02x", &val);
        key[i] = (unsigned char)val;
    }

    size_t data_len = strlen(data_str);
    unsigned char iv[12]; // 96-bit IV
    mbedtls_entropy_context entropy;
    mbedtls_ctr_drbg_context ctr_drbg;
    if (get_rng(&entropy, &ctr_drbg) != 0) return 0;
    mbedtls_ctr_drbg_random(&ctr_drbg, iv, sizeof(iv));
    free_rng(&entropy, &ctr_drbg);

    unsigned char tag[16]; // 128-bit tag
    unsigned char* out_buf = malloc(data_len);
    if (!out_buf) return 0;

    mbedtls_gcm_context gcm;
    mbedtls_gcm_init(&gcm);
    mbedtls_gcm_setkey(&gcm, MBEDTLS_CIPHER_ID_AES, key, 256);

    int ret = mbedtls_gcm_crypt_and_tag(&gcm, MBEDTLS_GCM_ENCRYPT, data_len, iv, sizeof(iv), NULL, 0,
                                        (const unsigned char*)data_str, out_buf, sizeof(tag), tag);
    mbedtls_gcm_free(&gcm);

    if (ret != 0) {
        free(out_buf);
        return 0;
    }

    // Format: IV(hex) + Tag(hex) + Ciphertext(hex)
    size_t res_len = (12 * 2) + (16 * 2) + (data_len * 2);
    char* res_str = llm_rt_alloc(res_len + 1, RT_TYPE_STRING);
    size_t offset = 0;
    for (int i = 0; i < 12; i++) { sprintf(res_str + offset, "%02x", iv[i]); offset += 2; }
    for (int i = 0; i < 16; i++) { sprintf(res_str + offset, "%02x", tag[i]); offset += 2; }
    for (size_t i = 0; i < data_len; i++) { sprintf(res_str + offset, "%02x", out_buf[i]); offset += 2; }
    res_str[res_len] = '\0';

    free(out_buf);
    return (long)res_str;
}

// AES-GCM Decrypt
long crypto_decrypt(long key_ptr, long enc_ptr) {
    char* key_hex = (char*)key_ptr;
    char* enc_hex = (char*)enc_ptr;
    if (!key_hex || !enc_hex) return 0;
    
    if (strlen(key_hex) != 64) return 0;

    size_t hex_len = strlen(enc_hex);
    size_t min_len = (12 * 2) + (16 * 2); // IV + Tag
    if (hex_len < min_len || hex_len % 2 != 0) return 0;

    unsigned char key[32];
    for (int i = 0; i < 32; i++) {
        unsigned int val;
        sscanf(key_hex + 2 * i, "%02x", &val);
        key[i] = (unsigned char)val;
    }

    unsigned char iv[12];
    unsigned char tag[16];
    size_t data_len = (hex_len - min_len) / 2;
    unsigned char* cipher = malloc(data_len);
    if (!cipher && data_len > 0) return 0;

    size_t offset = 0;
    for (int i = 0; i < 12; i++) { unsigned int val; sscanf(enc_hex + offset, "%02x", &val); iv[i] = val; offset += 2; }
    for (int i = 0; i < 16; i++) { unsigned int val; sscanf(enc_hex + offset, "%02x", &val); tag[i] = val; offset += 2; }
    for (size_t i = 0; i < data_len; i++) { unsigned int val; sscanf(enc_hex + offset, "%02x", &val); cipher[i] = val; offset += 2; }

    unsigned char* out_buf = malloc(data_len + 1);
    mbedtls_gcm_context gcm;
    mbedtls_gcm_init(&gcm);
    mbedtls_gcm_setkey(&gcm, MBEDTLS_CIPHER_ID_AES, key, 256);

    int ret = mbedtls_gcm_auth_decrypt(&gcm, data_len, iv, sizeof(iv), NULL, 0, tag, sizeof(tag), cipher, out_buf);
    mbedtls_gcm_free(&gcm);
    free(cipher);

    if (ret != 0) {
        free(out_buf);
        return 0;
    }

    out_buf[data_len] = '\0';
    char* res_str = llm_rt_alloc(data_len + 1, RT_TYPE_STRING);
    memcpy(res_str, out_buf, data_len + 1);
    free(out_buf);
    
    return (long)res_str;
}

void llm_drop_crypto_key(long s) {
    // Currently keys are passed as strings and automatically freed by RT_TYPE_STRING logic.
    // If we transition to caching parsed mbedtls_pk_contexts inside RT_TYPE_CRYPTO_KEY,
    // we would call mbedtls_pk_free() here.
}
