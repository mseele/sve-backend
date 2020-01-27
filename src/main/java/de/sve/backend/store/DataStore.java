package de.sve.backend.store;

import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutionException;
import java.util.logging.Logger;

import de.sve.backend.model.Event;

public class DataStore {

	private static final Logger LOG = Logger.getLogger(DataStore.class.getName());

	private static List<Event> EVENT_CACHE;

	private static LocalDateTime LAST_REFRESH;

	public static Event event(String id) throws Exception {
		List<Event> events = events();
		for (Event event : events) {
			if (id.equals(event.id())) {
				return event;
			}
		}
		return null;
	}

	public static List<Event> events() throws Exception {
		lazyLoad();
		return EVENT_CACHE;
	}

	public static void save(Event data) throws Exception {
		try (EventsStore store = new EventsStore()) {
			Event event = store.loadEvent(data.id());
			if (event != null) {
				event = event.update(data);
			} else {
				event = data;
			}
			store.saveEvent(event);
			reloadCache(store);
		}
	}

	public static void delete(Event event) throws Exception {
		try (EventsStore store = new EventsStore()) {
			store.deleteEvent(event);
			reloadCache(store);
		}
	}

	private static void lazyLoad() throws Exception {
		if (LAST_REFRESH == null || LAST_REFRESH.isBefore(LAST_REFRESH.minusMinutes(60))) {
			reloadCache();
		}
	}

	private static void reloadCache() throws Exception {
		try (EventsStore store = new EventsStore()) {
			reloadCache(store);
		}
	}

	private static void reloadCache(EventsStore store) throws InterruptedException, ExecutionException {
		EVENT_CACHE = new ArrayList<>(store.loadEvents());
		LAST_REFRESH = LocalDateTime.now();
		LOG.info("Event cache loaded successfully"); //$NON-NLS-1$
	}

}
