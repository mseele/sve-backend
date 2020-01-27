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

	public static Event event(String id) throws InterruptedException, ExecutionException {
		List<Event> events = events();
		for (Event event : events) {
			if (id.equals(event.id())) {
				return event;
			}
		}
		return null;
	}

	public static List<Event> events() throws InterruptedException, ExecutionException {
		lazyLoad();
		return EVENT_CACHE;
	}

	public static void save(Event event) throws InterruptedException, ExecutionException {
		EventsStore.saveEvent(event);
		LAST_REFRESH = null;
	}

	public static void delete(Event event) throws InterruptedException, ExecutionException {
		EventsStore.deleteEvent(event);
		LAST_REFRESH = null;
	}

	private static void lazyLoad() throws InterruptedException, ExecutionException {
		if (LAST_REFRESH == null || LAST_REFRESH.isBefore(LAST_REFRESH.minusMinutes(60))) {
			reloadCache();
		}
	}

	private static void reloadCache() throws InterruptedException, ExecutionException {
		EVENT_CACHE = new ArrayList<>(EventsStore.loadEvents());
		LAST_REFRESH = LocalDateTime.now();
		LOG.info("Event cache loaded successfully"); //$NON-NLS-1$
	}

}
