part of 'torrents_bloc.dart';

@freezed
class TorrentsEvent with _$TorrentsEvent {
  const factory TorrentsEvent.started() = _Started;
  const factory TorrentsEvent.fetch() = _Fetch;
}
