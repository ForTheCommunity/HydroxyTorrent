part of 'torrents_bloc.dart';

@freezed
class TorrentsState with _$TorrentsState {
  const factory TorrentsState.initial() = _Initial;
  const factory TorrentsState.loading() = _Loading;
  const factory TorrentsState.loaded({
    required List<TorrentStructure> torrents,
  }) = _Loaded;
}
