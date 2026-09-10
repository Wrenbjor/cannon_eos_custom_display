// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <windows.h>
#include <string>
#include <vector>
#include <algorithm>
#include <cstdint>

inline constexpr GUID OpenEosPipeAttribute = {0x921f5ac1,0x3d6b,0x4ff6,{0x8e,0x31,0x23,0x56,0xa6,0x3a,0xa1,0x13}};
inline constexpr GUID OpenEosTestAttribute = {0x921f5ac2,0x3d6b,0x4ff6,{0x8e,0x31,0x23,0x56,0xa6,0x3a,0xa1,0x13}};
struct OpenEosFrameHeader {
    uint32_t magic, version, width, height, length, live;
    uint64_t sequence, tick;
};
static_assert(sizeof(OpenEosFrameHeader) == 40);

class FrameBridge {
    std::wstring m_pipe;
    static bool transfer(HANDLE pipe, BYTE* bytes, DWORD length, bool write, ULONGLONG deadline) {
        DWORD total = 0;
        while (total < length) {
            OVERLAPPED overlap{};
            BOOL ok = write ? WriteFile(pipe, bytes + total, length - total, nullptr, &overlap)
                            : ReadFile(pipe, bytes + total, length - total, nullptr, &overlap);
            if (!ok && GetLastError() != ERROR_IO_PENDING) return false;
            DWORD transferred = 0;
            const ULONGLONG now = GetTickCount64();
            if (now >= deadline || !GetOverlappedResultEx(pipe, &overlap, &transferred, DWORD(deadline - now), FALSE)) {
                CancelIoEx(pipe, &overlap);
                GetOverlappedResult(pipe, &overlap, &transferred, TRUE);
                return false;
            }
            if (!transferred) return false;
            total += transferred;
        }
        return true;
    }
public:
    void setPipe(const std::wstring& pipe) {
        const std::wstring prefix = L"\\\\.\\pipe\\OpenEOSCamera-";
        // Attributes must never redirect the service to a remote pipe or file.
        if (pipe.compare(0, prefix.size(), prefix) != 0 || pipe.size() != prefix.size() + 36) return;
        for (size_t i = prefix.size(); i < pipe.size(); ++i) {
            const wchar_t c = pipe[i];
            if (!(c == L'-' || (c >= L'0' && c <= L'9') || (c >= L'A' && c <= L'F') || (c >= L'a' && c <= L'f'))) return;
        }
        m_pipe = pipe;
    }
    bool read(OpenEosFrameHeader& header, std::vector<BYTE>& rgb) {
        if (m_pipe.empty()) return false;
        HANDLE pipe = CreateFileW(m_pipe.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, nullptr);
        if (pipe == INVALID_HANDLE_VALUE) return false;
        const ULONGLONG deadline = GetTickCount64() + 200;
        bool ok = transfer(pipe, reinterpret_cast<BYTE*>(&header), sizeof(header), false, deadline);
        ok = ok && header.magic == 0x31534f45 && header.version == 1 && header.length <= 16 * 1024 * 1024;
        if (ok && header.live == 1) {
            ok = header.width >= 2 && header.height >= 2 && header.width <= 4096 && header.height <= 4096
                && uint64_t(header.width) * header.height * 3 == header.length;
            if (ok) { rgb.resize(header.length); ok = transfer(pipe, rgb.data(), header.length, false, deadline); }
        } else { ok = ok && header.live == 0 && header.length == 0; }
        BYTE ack = 1;
        if (ok) ok = transfer(pipe, &ack, 1, true, deadline);
        CloseHandle(pipe);
        const auto now = GetTickCount64();
        return ok && header.live == 1 && now >= header.tick && now - header.tick <= 1000;
    }
    void render(BYTE* pixels, LONG pitch, UINT32 width, UINT32 height) {
        for (UINT32 y = 0; y < height; ++y) memset(pixels + size_t(y) * pitch, 0, size_t(width) * 4);
        OpenEosFrameHeader header{};
        std::vector<BYTE> rgb;
        if (!read(header, rgb)) return; // Never expose a stale frame after disconnect.
        UINT32 dw = width, dh = UINT32(uint64_t(width) * header.height / header.width);
        if (dh > height) { dh = height; dw = UINT32(uint64_t(height) * header.width / header.height); }
        const UINT32 left = (width - dw) / 2, top = (height - dh) / 2;
        for (UINT32 y = 0; y < dh; ++y) {
            BYTE* row = pixels + size_t(y + top) * pitch + left * 4;
            const UINT32 sy = UINT32(uint64_t(y) * header.height / dh);
            for (UINT32 x = 0; x < dw; ++x) {
                const UINT32 sx = UINT32(uint64_t(x) * header.width / dw);
                const BYTE* src = rgb.data() + (size_t(sy) * header.width + sx) * 3;
                row[x * 4] = src[2]; row[x * 4 + 1] = src[1]; row[x * 4 + 2] = src[0];
            }
        }
    }
};
