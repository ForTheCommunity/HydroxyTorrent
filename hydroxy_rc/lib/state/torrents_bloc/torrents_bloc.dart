import 'dart:async';

import 'package:bloc/bloc.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:meta/meta.dart';
import 'package:freezed_annotation/freezed_annotation.dart';

part 'torrents_event.dart';
part 'torrents_state.dart';
part 'torrents_bloc.freezed.dart';

class TorrentsBloc extends Bloc<TorrentsEvent, TorrentsState> {
  Timer? _pollingTimer;

  TorrentsBloc() : super(_Initial()) {
    on<_Fetch>(_fetchTorrents);

    // start polling
    _startPolling();
  }

  Future<void> _fetchTorrents(_Fetch event, Emitter<TorrentsState> emit) async {
    // Only poll when authenticated with the daemon
    if (!ApiClient.instance.isAuthenticated) {
      return;
    }

    if (state is! _Loaded) {
      emit(_Loading());
    }

    try {
      var torrentsData = await getTorrents();
      emit(_Loaded(torrents: torrentsData));
    } catch (e) {
      print("TorrentsBloc fetch error: ${formatConnectionError(e)}");
    }
  }

  void _startPolling() {
    _pollingTimer = Timer.periodic(const Duration(seconds: 2), (timer) {
      add(const TorrentsEvent.fetch());
    });
  }

  @override
  Future<void> close() {
    // Cancel the timer when the bloc is destroyed to prevent memory leaks
    _pollingTimer?.cancel();
    return super.close();
  }
}
