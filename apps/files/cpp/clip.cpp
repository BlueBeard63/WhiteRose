#include "whiterose-files/cpp/clip.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QMimeData>
#include <QUrl>

void setClipboardFiles(rust::Str uris, bool cut)
{
    const QString all = QString::fromUtf8(uris.data(), qsizetype(uris.size()));
    QList<QUrl> urls;
    QStringList paths;
    QByteArray gnome = cut ? "cut" : "copy";
    for (const QString &line : all.split(u'\n', Qt::SkipEmptyParts)) {
        const QUrl url(line);
        urls << url;
        paths << url.toLocalFile();
        gnome += '\n' + url.toEncoded();
    }
    auto *data = new QMimeData;
    data->setUrls(urls);
    data->setText(paths.join(u'\n'));
    data->setData(QStringLiteral("x-special/gnome-copied-files"), gnome);
    // KDE's way of saying "cut".
    data->setData(QStringLiteral("application/x-kde-cutselection"), cut ? "1" : "0");
    QGuiApplication::clipboard()->setMimeData(data);
}

rust::String clipboardFiles()
{
    const QMimeData *data = QGuiApplication::clipboard()->mimeData();
    if (!data)
        return rust::String();
    QByteArray out;
    const QByteArray gnome = data->data(QStringLiteral("x-special/gnome-copied-files"));
    if (!gnome.isEmpty()) {
        out = gnome;
    } else if (data->hasUrls()) {
        const bool cut = data->data(QStringLiteral("application/x-kde-cutselection")) == "1";
        out = cut ? "cut" : "copy";
        for (const QUrl &u : data->urls())
            if (u.isLocalFile())
                out += '\n' + u.toEncoded();
    }
    return rust::String(out.constData(), size_t(out.size()));
}
