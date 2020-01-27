package de.sve.backend.store;

import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;
import java.util.stream.Collectors;

import com.google.api.core.ApiFuture;
import com.google.cloud.firestore.CollectionReference;
import com.google.cloud.firestore.Firestore;
import com.google.cloud.firestore.FirestoreOptions;
import com.google.cloud.firestore.QueryDocumentSnapshot;
import com.google.cloud.firestore.QuerySnapshot;

import de.sve.backend.model.Event;
import de.sve.backend.model.EventType;

public class EventsStore {

	protected static List<Event> loadEvents() throws InterruptedException, ExecutionException {
		List<Event> events = new ArrayList<>();
		ApiFuture<QuerySnapshot> query = collection().get();
		QuerySnapshot querySnapshot = query.get();
		for (QueryDocumentSnapshot document : querySnapshot.getDocuments()) {
			@SuppressWarnings("unchecked")
			List<String> dates = document.get("dates", List.class); //$NON-NLS-1$
			events.add(Event.create(document.getId(),
									document.getString("sheetId"), //$NON-NLS-1$
									document.getLong("gid"), //$NON-NLS-1$
									EventType.valueOf(document.getString("type")), //$NON-NLS-1$
									document.getString("shortName"), //$NON-NLS-1$
									document.getString("name"), //$NON-NLS-1$
									document.getLong("sortIndex"), //$NON-NLS-1$
									document.getBoolean("visible"), //$NON-NLS-1$
									document.getString("shortDescription"), //$NON-NLS-1$
									document.getString("description"), //$NON-NLS-1$
									document.getString("image"), //$NON-NLS-1$
									document.getString("titleColor"), //$NON-NLS-1$
									dates.stream().map(LocalDateTime::parse).collect(Collectors.toList()),
									document.getLong("durationInMinutes"), //$NON-NLS-1$
									document.getLong("maxSubscribers"), //$NON-NLS-1$
									document.getLong("subscribers"), //$NON-NLS-1$
									document.getDouble("costMember"), //$NON-NLS-1$
									document.getDouble("costNonMember"), //$NON-NLS-1$
									document.getLong("waitingList"), //$NON-NLS-1$
									document.getLong("maxWaitingList"), //$NON-NLS-1$
									document.getString("location"), //$NON-NLS-1$
									document.getString("bookingTemplate"), //$NON-NLS-1$
									document.getString("waitingTemplate"))); //$NON-NLS-1$
		}
		return events;
	}

	protected static void saveEvent(Event event) throws InterruptedException, ExecutionException {
		List<String> dates = event.dates()
								  .stream()
								  .map(LocalDateTime::toString)
								  .collect(Collectors.toList());
		Map<String, Object> data = new HashMap<>();
		data.put("sheetId", event.sheetId()); //$NON-NLS-1$
		data.put("gid", event.gid()); //$NON-NLS-1$
		data.put("type", event.type().name()); //$NON-NLS-1$
		data.put("shortName", event.shortName()); //$NON-NLS-1$
		data.put("name",event.name()); //$NON-NLS-1$
		data.put("sortIndex", event.sortIndex()); //$NON-NLS-1$
		data.put("visible", event.visible()); //$NON-NLS-1$
		data.put("shortDescription", event.shortDescription()); //$NON-NLS-1$
		data.put("description", event.description()); //$NON-NLS-1$
		data.put("image", event.image()); //$NON-NLS-1$
		data.put("titleColor", event.titleColor()); //$NON-NLS-1$
		data.put("dates", dates); //$NON-NLS-1$
		data.put("durationInMinutes", event.durationInMinutes()); //$NON-NLS-1$
		data.put("maxSubscribers", event.maxSubscribers()); //$NON-NLS-1$
		data.put("subscribers", event.subscribers()); //$NON-NLS-1$
		data.put("costMember", event.costMember()); //$NON-NLS-1$
		data.put("costNonMember", event.costNonMember()); //$NON-NLS-1$
		data.put("waitingList", event.waitingList()); //$NON-NLS-1$
		data.put("maxWaitingList", event.maxWaitingList()); //$NON-NLS-1$
		data.put("location", event.location()); //$NON-NLS-1$
		data.put("bookingTemplate", event.bookingTemplate()); //$NON-NLS-1$
		data.put("waitingTemplate", event.waitingTemplate()); //$NON-NLS-1$
		collection().document(event.id()).set(data).get();
	}

	protected static void deleteEvent(Event event) throws InterruptedException, ExecutionException {
		collection().document(event.id()).delete().get();
	}

	private static CollectionReference collection() {
		return db().collection("events"); //$NON-NLS-1$
	}

	private static Firestore db() {
		return FirestoreOptions.getDefaultInstance().getService();
	}

}
